"""Real process receipts retain complete case, side and budget identities."""

# CI discovers these unittest controls before any optional pytest installation.
# ruff: noqa: PT009

import hashlib
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Final

from scripts.world_check_support import read_json

PRODUCER: Final = """
import importlib
import sys
from pathlib import Path
producer = importlib.import_module("scripts.compat.script-vm.tables.compare")
producer.run(sys.argv[2:], Path(sys.argv[1]))
"""
EMITTER: Final = """
import sys
print("stdout:" + sys.argv[1])
print("stderr:" + sys.argv[1], file=sys.stderr)
sys.exit(int(sys.argv[2]))
"""


def check_process_receipts(
    testcase: unittest.TestCase, tmp_path: Path, case: str
) -> None:
    requests: list[tuple[str, list[str], int]] = []
    for budget in range(3):
        for side in ("native", "rust"):
            name = f"{case}-{budget}-{side}"
            status = 7 if budget == 2 else 0
            argv = [sys.executable, "-c", EMITTER, name, str(status)]
            result = subprocess.run(
                [sys.executable, "-c", PRODUCER, str(tmp_path / name), *argv],
                capture_output=True,
                check=False,
                timeout=30,
            )
            testcase.assertEqual(result.returncode, 1 if status else 0, result.stderr)
            requests.append((name, argv, status))
    expected = {
        f"{name}.{extension}"
        for name, _, _ in requests
        for extension in ("stdout", "stderr", "json")
    }
    testcase.assertEqual({path.name for path in tmp_path.iterdir()}, expected)
    for name, argv, status in requests:
        stdout = f"stdout:{name}\n".encode()
        stderr = f"stderr:{name}\n".encode()
        testcase.assertEqual((tmp_path / f"{name}.stdout").read_bytes(), stdout)
        testcase.assertEqual((tmp_path / f"{name}.stderr").read_bytes(), stderr)
        testcase.assertEqual(
            read_json(tmp_path / f"{name}.json"),
            {
                "argv": argv,
                "status": status,
                "stdout_sha256": hashlib.sha256(stdout).hexdigest(),
                "stderr_sha256": hashlib.sha256(stderr).hexdigest(),
            },
        )


POSTCONDITION: Final = """
import importlib
import json
import sys
from dataclasses import replace
from pathlib import Path
producer = importlib.import_module("scripts.compat.script-vm.tables.compare")
root = Path(sys.argv[1])
mutation = sys.argv[2]
for side in ("native", "rust"):
    binary = root / side
    binary.write_text("#!/bin/sh\\nprintf 'return 99 integer 1\\\\n'\\n")
    binary.chmod(0o700)
scenarios = [producer.Case("scalar-1.0", "return 1;"),
             producer.Case("plain", "return 2;")]
records = []
for case in scenarios:
    source = root / (case.name + ".nut")
    source.write_text(case.source)
    for number, credits in enumerate(producer.BUDGETS):
        for side in ("native", "rust"):
            prefix = root / f"{case.name}-{number}-{side}"
            producer.run([str(root / side), str(source), *map(str, credits)], prefix)
        stdout = root / f"{case.name}-{number}-native.stdout"
        records.append(producer.Observation(case.name, number, credits, source.name,
            f"{case.name}-{number}-native", f"{case.name}-{number}-rust",
            producer.digest(source), producer.digest(stdout)))
producer.verify_retention(root, scenarios, records)
print("baseline verified")
name = "scalar-1.0-0-native"
receipt_path = root / (name + ".json")
if mutation == "missing":
    (root / (name + ".stdout")).unlink()
elif mutation == "extra":
    (root / "orphan-native.json").write_text("{}")
elif mutation == "duplicate":
    records[-1] = records[0]
elif mutation == "source-identity":
    scenarios.append(scenarios[0])
elif mutation == "prefix":
    records[0] = replace(records[0], native_prefix=records[1].native_prefix)
elif mutation == "source":
    (root / "scalar-1.0.nut").write_text("return 3;")
elif mutation == "budget":
    records[0] = replace(records[0], credits=(999,))
elif mutation in ("stdout", "stderr"):
    (root / (name + "." + mutation)).write_text("changed")
elif mutation in ("status", "hash", "argv"):
    receipt = json.loads(receipt_path.read_text())
    if mutation == "status": receipt["status"] = 7
    if mutation == "hash": receipt["stdout_sha256"] = "0" * 64
    if mutation == "argv": receipt["argv"][-1] = "999"
    receipt_path.write_text(json.dumps(receipt))
elif mutation == "swapped":
    receipt_path.write_bytes((root / "scalar-1.0-1-rust.json").read_bytes())
elif mutation == "record-hash":
    records[0] = replace(records[0], stdout_sha256="0" * 64)
elif mutation == "coherent-stage":
    for side in ("native", "rust"):
        stem = "scalar-1.0-0-" + side
        stdout = root / (stem + ".stdout")
        stdout.write_text("compile_error\\n")
        path = root / (stem + ".json")
        receipt = json.loads(path.read_text())
        receipt["stdout_sha256"] = producer.digest(stdout)
        path.write_text(json.dumps(receipt))
    records[0] = replace(records[0], stdout_sha256=producer.digest(stdout))
else:
    raise AssertionError(mutation)
try:
    producer.verify_retention(root, scenarios, records)
except RuntimeError:
    print("rejected:" + mutation)
else:
    raise AssertionError("retention corruption accepted:" + mutation)
"""


class TableStorageTest(unittest.TestCase):
    def test_real_process_receipts_preserve_dotted_and_plain_prefixes(self) -> None:
        for case in ("delete-scalar-1.0", "delete-scalar--0.0", "dot-free"):
            with self.subTest(case=case), tempfile.TemporaryDirectory() as directory:
                check_process_receipts(self, Path(directory), case)

    def test_complete_receipt_postcondition_rejects_corruptions(self) -> None:
        for mutation in (
            "missing",
            "extra",
            "duplicate",
            "prefix",
            "source",
            "budget",
            "stdout",
            "stderr",
            "status",
            "hash",
            "argv",
            "swapped",
            "record-hash",
            "source-identity",
            "coherent-stage",
        ):
            with (
                self.subTest(mutation=mutation),
                tempfile.TemporaryDirectory() as directory,
            ):
                result = subprocess.run(
                    [sys.executable, "-c", POSTCONDITION, directory, mutation],
                    capture_output=True,
                    check=False,
                    timeout=30,
                )
                self.assertEqual(result.returncode, 0, result.stderr.decode())
                self.assertEqual(
                    result.stdout, f"baseline verified\nrejected:{mutation}\n".encode()
                )


if __name__ == "__main__":
    _ = unittest.main()
