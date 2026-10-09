"""Contract failures must be observable before compatibility commands execute."""

import copy
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))


class ContractTests(unittest.TestCase):
    def test_valid_contract_and_rejection_controls(self):
        from contract_validate import ContractError, load_contract

        contract = json.loads((ROOT / "compatibility/contract.json").read_text())
        load_contract(ROOT / "compatibility/contract.json", ROOT)
        mutations = [
            ("scope", lambda d: d["drivers"][0].update(scopes=["invalid-scope"])),
            (
                "pin",
                lambda d: d["target"]["base_graphics"].update(archive_sha256="0" * 64),
            ),
            ("schema_version", lambda d: d.update(schema_version=2)),
            ("pin", lambda d: d["target"].update(commit="0" * 40)),
            ("sha256", lambda d: d["assets"][0].update(sha256="0" * 64)),
            ("duplicate", lambda d: d["scenarios"].append(d["scenarios"][0])),
            ("driver", lambda d: d["scenarios"][0].update(driver="absent")),
            ("status", lambda d: d["scenarios"][0].update(status="supported")),
            ("scope", lambda d: d["scenarios"][0].update(scope="original_only")),
            (
                "unimplemented",
                lambda d: d["scenarios"][0].update(
                    status="unimplemented", scope="future"
                ),
            ),
            (
                "source",
                lambda d: d["scenarios"][0].update(
                    source_refs=["README.md#nonexistent-contract-symbol"]
                ),
            ),
            ("asset", lambda d: d["scenarios"][0].update(assets=["absent"])),
            ("path", lambda d: d["assets"][0].update(path="../outside.sav")),
            ("scope", lambda d: d["scenarios"][0].update(driver="native-contract")),
            ("timeout", lambda d: d["drivers"][0].update(timeout_seconds=0)),
            ("unknown", lambda d: d.update(typo=True)),
        ]
        mutations.append(
            (
                "profile",
                lambda d: next(a for a in d["assets"] if a["id"] == "native-0").update(
                    profile="vanilla"
                ),
            )
        )
        mutations.extend(
            [
                (
                    "content identity",
                    lambda d: next(p for p in d["profiles"] if p["id"] == "modded")[
                        "content"
                    ][0].update(grfid="00000000"),
                ),
                (
                    "content identity",
                    lambda d: next(p for p in d["profiles"] if p["id"] == "modded")[
                        "content"
                    ][0].update(parameters=[42]),
                ),
                (
                    "content identity",
                    lambda d: next(p for p in d["profiles"] if p["id"] == "modded").update(
                        content=[]
                    ),
                ),
            ]
        )
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "contract.json"
            for reason, mutate in mutations:
                with self.subTest(reason=reason):
                    changed = copy.deepcopy(contract)
                    mutate(changed)
                    path.write_text(json.dumps(changed))
                    with self.assertRaisesRegex(ContractError, reason):
                        load_contract(path, ROOT)
            path.write_bytes(b" " * (1024 * 1024 + 1))
            with self.assertRaisesRegex(ContractError, "byte limit"):
                load_contract(path, ROOT)
            path.write_text('{"schema_version":1,"schema_version":1}')
            with self.assertRaisesRegex(ContractError, "duplicate"):
                load_contract(path, ROOT)

    def test_real_process_failure_missing_evidence_and_freshness(self):
        from contract_model import Driver, Scope
        from contract_run import run_driver

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "outputs").mkdir()
            for name, code, expected in [
                (
                    "success",
                    "from pathlib import Path; p=Path('outputs/new'); p.mkdir(); (p/'proof.txt').write_text('actual evidence'); print('Artifacts: '+str(p.resolve())); print('PASS')",
                    True,
                ),
                ("failure", "print('PASS'); raise SystemExit(9)", False),
                ("missing", "print('PASS')", False),
                (
                    "missing-case",
                    "from pathlib import Path; p=Path('outputs/partial'); p.mkdir(); (p/'proof.txt').write_text('one of two cases'); print('Artifacts: '+str(p.resolve())); print('PASS')",
                    False,
                ),
                (
                    "stale",
                    "from pathlib import Path; print('Artifacts: '+str(Path('outputs/new').resolve())); print('PASS')",
                    False,
                ),
                (
                    "escape",
                    "from pathlib import Path; p=Path('outputs/escape'); p.mkdir(); (p/'proof.txt').symlink_to(Path('success/stdout.log').resolve()); print('Artifacts: '+str(p.resolve())); print('PASS')",
                    False,
                ),
                ("timeout", "import time; time.sleep(30)", False),
                (
                    "empty",
                    "from pathlib import Path; p=Path('outputs/empty'); p.mkdir(); (p/'proof.txt').touch(); print('Artifacts: '+str(p.resolve())); print('PASS')",
                    False,
                ),
            ]:
                with self.subTest(name=name):
                    driver = Driver(
                        name,
                        (sys.executable, "-c", code),
                        "outputs",
                        ("proof.txt", "second-case.txt")
                        if name == "missing-case"
                        else ("proof.txt",),
                        "PASS",
                        (Scope.RUST,),
                        2,
                    )
                    result = run_driver(driver, root, root / name)
                    self.assertEqual(result.passed, expected)
                    self.assertTrue((root / name / "stdout.log").is_file())
            failed = json.loads((root / "failure" / "result.json").read_text())
            self.assertEqual(failed["exit_code"], 9)

    def test_report_retains_manifest_identity_and_unrun_requirements(self):
        from contract_validate import load_contract
        from contract_run import run_contract

        with tempfile.TemporaryDirectory() as directory:
            temporary = Path(directory)
            raw = json.loads((ROOT / "compatibility/contract.json").read_text())
            raw["drivers"][0]["argv"] = [
                sys.executable,
                "-c",
                "print('test result: ok.')",
            ]
            path = temporary / "alternate.json"
            path.write_text(json.dumps(raw))
            contract = load_contract(path, ROOT)
            self.assertTrue(
                run_contract(contract, "rust-tests", ROOT, temporary / "report")
            )
            report = json.loads((temporary / "report/report.json").read_text())
            self.assertEqual(report["manifest_path"], str(path.resolve()))
            self.assertEqual(
                report["manifest_sha256"], hashlib.sha256(path.read_bytes()).hexdigest()
            )
            self.assertEqual(len(report["drivers"]), 1)
            for scenario in report["scenarios"]:
                self.assertEqual(
                    scenario["run_status"],
                    "pass" if scenario["driver"] == "rust-tests" else "not_run",
                )
            with self.assertRaises(FileExistsError):
                run_contract(contract, "rust-tests", ROOT, temporary / "report")

    def test_cli_validation_is_not_execution(self):
        command = [sys.executable, str(ROOT / "scripts/check-contract.py")]
        with tempfile.TemporaryDirectory() as directory:
            temporary = Path(directory)
            contract = json.loads((ROOT / "compatibility/contract.json").read_text())
            sentinel = temporary / "executed"
            for driver in contract["drivers"]:
                driver["argv"] = [
                    sys.executable,
                    "-c",
                    f"from pathlib import Path; Path({str(sentinel)!r}).touch()",
                ]
            path = temporary / "contract.json"
            path.write_text(json.dumps(contract))
            checked = subprocess.run(
                [*command, "--validate", "--contract", str(path)],
                cwd=ROOT,
                capture_output=True,
                text=True,
            )
            self.assertFalse(sentinel.exists(), "validation executed a driver")
        self.assertEqual(checked.returncode, 0, checked.stderr)
        self.assertIn("no scenarios executed", checked.stdout)
        future = subprocess.run(
            [*command, "--run", "network.rust-original"],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        self.assertNotEqual(future.returncode, 0)
        self.assertIn("unimplemented requirement; no execution", future.stderr)


if __name__ == "__main__":
    unittest.main()
