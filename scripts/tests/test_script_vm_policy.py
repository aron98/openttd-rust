"""Admission controls for explicit undefined-native-input fixture policy."""

import os
import shutil
import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from scripts.script_vm_policy import (
    POLICY_STAGE,
    UNDEFINED_INPUTS,
    compare_case,
    observe_native,
)
from scripts.script_vm_provenance import load_spec
from scripts.world_check_support import ROOT, WorldCheckError, read_json, write_json


class ScriptVmPolicyTests(unittest.TestCase):
    def test_reached_hex_lookahead_has_explicit_policy_metadata(self) -> None:
        for prefix in (b'return "\\x', b'return "\\x1', b'return "\\x1234'):
            for payload, point in (
                (b"\xc4\x80", 256),
                (b"\xe2\x82\xac", 0x20AC),
                (b"\xed\xa0\x80", 0xD800),
            ):
                source = prefix + payload + b'";'
                self.assertIn(source, UNDEFINED_INPUTS)
                policy = UNDEFINED_INPUTS[source]
                self.assertEqual(
                    (policy.codepoint, policy.context, policy.offset),
                    (point, "HexEscape", len(prefix)),
                )

    def test_hex_admission_requires_context_offset_and_unchanged_source(self) -> None:
        with TemporaryDirectory() as temporary:
            case = Path(temporary)
            (case / "rust").mkdir()
            _ = (case / "rust/stdout.log").write_text("compile_error\n")
            write_json(
                case / "policy.json", {"native_executed": False, "reason": POLICY_STAGE}
            )
            write_json(case / "rust/process.json", {"returncode": 0, "expected": 0})
            for source, policy in UNDEFINED_INPUTS.items():
                if policy.context != "HexEscape":
                    continue
                diagnostic = policy.diagnostic()
                _ = (case / "rust/stderr.log").write_text(diagnostic)
                compare_case(source, case, POLICY_STAGE)
                for changed in (
                    diagnostic.replace("HexEscape", "Token"),
                    diagnostic.replace(
                        f"byte {policy.offset}", f"byte {policy.offset + 1}"
                    ),
                ):
                    _ = (case / "rust/stderr.log").write_text(changed)
                    with self.assertRaises(WorldCheckError):
                        compare_case(source, case, POLICY_STAGE)
                _ = (case / "rust/stderr.log").write_text(diagnostic)
                with self.assertRaises(WorldCheckError):
                    compare_case(source + b" ", case, POLICY_STAGE)

    def test_manifest_classifies_only_exact_source_bodies(self) -> None:
        spec = load_spec(ROOT / "scripts/script-vm-manifest.json")
        stages = dict(spec.stages)
        found: set[bytes] = set()
        for name in spec.fixtures:
            source = (ROOT / "scripts/compat/script-vm/fixtures" / name).read_bytes()
            policy = UNDEFINED_INPUTS.get(source)
            for schedule in spec.credits:
                key = f"{Path(name).stem}/{'-'.join(map(str, schedule))}"
                self.assertEqual(stages[key] == POLICY_STAGE, policy is not None, key)
            if policy is not None:
                found.add(source)
        self.assertEqual(found, set(UNDEFINED_INPUTS))

    def test_only_exact_undefined_bodies_are_classified(self) -> None:
        self.assertEqual(len(UNDEFINED_INPUTS), 17)
        for payload in (b"\xc3\xa9", b"\xc4\x80", b"\xed\xa0\x80", b"\xff"):
            for source in (
                b"return/*" + payload + b"*/1;\n",
                b"return//" + payload + b"\n1;\n",
                b"return 1;\0" + payload,
                b'return "' + payload + b'";',
            ):
                self.assertNotIn(source, UNDEFINED_INPUTS)
        self.assertNotIn(b"return 1;/*x*/\xc3\xa9\n", UNDEFINED_INPUTS)
        self.assertNotIn(b"return 1;/*x*/\xff\n", UNDEFINED_INPUTS)

    def test_defined_comparison_never_uses_policy_exemption(self) -> None:
        with TemporaryDirectory() as temporary:
            case = Path(temporary)
            (case / "native").mkdir()
            (case / "rust").mkdir()
            _ = (case / "native/stdout.log").write_text("compile_error\n")
            _ = (case / "rust/stdout.log").write_text("compile_error\n")
            _ = (case / "rust/stderr.log").write_text("UnsupportedSyntax at byte 7\n")
            source = b"return \xc3\xa9;"
            compare_case(source, case, "compile_error")
            with self.assertRaises(WorldCheckError):
                compare_case(source, case, POLICY_STAGE)
            _ = (case / "native/stdout.log").write_text("return 9998 integer 1\n")
            with self.assertRaises(WorldCheckError):
                compare_case(source, case, "compile_error")
            _ = (case / "native/stdout.log").write_text("compile_error\n")
            _ = (case / "rust/stderr.log").write_text("UndefinedNativeCharacter\n")
            with self.assertRaises(WorldCheckError):
                compare_case(source, case, "compile_error")

    def test_policy_requires_exact_typed_rejection_without_native(self) -> None:
        source = b"return 1;/*x*/\xed\xa0\x80\n"
        diagnostic = (
            "UndefinedNativeCharacter { codepoint: 55296, context: Token } at byte 14\n"
        )
        with TemporaryDirectory() as temporary:
            case = Path(temporary)
            (case / "rust").mkdir()
            _ = (case / "rust/stdout.log").write_text("compile_error\n")
            _ = (case / "rust/stderr.log").write_text(diagnostic)
            write_json(case / "rust/process.json", {"returncode": 0, "expected": 0})
            write_json(
                case / "policy.json", {"native_executed": False, "reason": POLICY_STAGE}
            )
            compare_case(source, case, POLICY_STAGE)
            for changed in (
                "UnsupportedSyntax at byte 14\n",
                diagnostic.replace("55296", "256"),
                diagnostic.replace("14", "13"),
                diagnostic.replace("Token", "Identifier"),
            ):
                _ = (case / "rust/stderr.log").write_text(changed)
                with self.assertRaises(WorldCheckError):
                    compare_case(source, case, POLICY_STAGE)
            _ = (case / "rust/stderr.log").write_text(diagnostic)
            with self.assertRaises(WorldCheckError):
                compare_case(source, case, "compile_error")
            _ = (case / "rust/stdout.log").write_text("return 9998 integer 1\n")
            with self.assertRaises(WorldCheckError):
                compare_case(source, case, POLICY_STAGE)
            _ = (case / "rust/stdout.log").write_text("compile_error\n")
            write_json(case / "rust/process.json", {"returncode": 1, "expected": 0})
            with self.assertRaises(WorldCheckError):
                compare_case(source, case, POLICY_STAGE)

    def test_undefined_producer_never_invokes_native(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            marker = root / "native-invoked"
            argv = [
                sys.executable,
                "-c",
                "from pathlib import Path; import sys; Path(sys.argv[1]).touch()",
                str(marker),
            ]
            for index, source in enumerate(UNDEFINED_INPUTS):
                case = root / str(index)
                observe_native(argv, case, source)
                self.assertFalse(marker.exists())
                self.assertFalse((case / "native").exists())
                self.assertEqual(
                    read_json(case / "policy.json"),
                    {"native_executed": False, "reason": POLICY_STAGE},
                )

    def test_policy_receipt_corruption_and_native_path_are_rejected(self) -> None:
        source, policy = next(iter(UNDEFINED_INPUTS.items()))
        with TemporaryDirectory() as temporary:
            case = Path(temporary)
            (case / "rust").mkdir()
            _ = (case / "rust/stdout.log").write_text("compile_error\n")
            _ = (case / "rust/stderr.log").write_text(policy.diagnostic())
            write_json(case / "rust/process.json", {"returncode": 0, "expected": 0})
            for receipt in (
                {},
                {"native_executed": True, "reason": POLICY_STAGE},
                {"native_executed": 0, "reason": POLICY_STAGE},
                {"native_executed": False, "reason": "return"},
                {"native_executed": False, "reason": POLICY_STAGE, "extra": 1},
            ):
                with self.subTest(receipt=receipt):
                    write_json(case / "policy.json", receipt)
                    with self.assertRaises(WorldCheckError):
                        compare_case(source, case, POLICY_STAGE)
            write_json(
                case / "policy.json", {"native_executed": False, "reason": POLICY_STAGE}
            )
            compare_case(source, case, POLICY_STAGE)
            (case / "native").mkdir()
            with self.assertRaises(WorldCheckError):
                compare_case(source, case, POLICY_STAGE)
            (case / "native").rmdir()
            (case / "native").symlink_to(case / "missing")
            with self.assertRaises(WorldCheckError):
                compare_case(source, case, POLICY_STAGE)

    def test_legacy_shell_never_invokes_native_for_policy_input(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            harness = root / "scripts/compat/script-vm"
            (harness / "fixtures").mkdir(parents=True)
            source, policy = next(iter(UNDEFINED_INPUTS.items()))
            _ = (harness / "fixtures/undefined.nut").write_bytes(source)
            shell = harness / "differential.sh"
            _ = shutil.copy2(ROOT / "scripts/compat/script-vm/differential.sh", shell)
            native = root / "native"
            marker = root / "native-invoked"
            _ = native.write_text(
                f"#!/bin/sh\ntouch '{marker}'\nprintf 'compile_error\\n'\n"
            )
            native.chmod(0o755)
            rust = root / "rust"
            _ = rust.write_text(
                "#!/bin/sh\nprintf 'compile_error\\n'\n"
                f"printf '%s' '{policy.diagnostic()}' >&2\n"
            )
            rust.chmod(0o755)
            result = subprocess.run(
                ["/bin/sh", str(shell), str(native), str(rust), str(root / "out")],
                capture_output=True,
                text=True,
                check=False,
                env={**os.environ, "PYTHONPATH": str(ROOT)},
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(marker.exists())
            self.assertEqual(len(list((root / "out").rglob("policy.json"))), 9)

    def test_defined_unicode_strings_and_comments_invoke_native(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            marker = root / "native-invoked"
            argv = [
                sys.executable,
                "-c",
                "from pathlib import Path; import sys; Path(sys.argv[1]).touch()",
                str(marker),
            ]
            for index, source in enumerate(
                ('return "€";', "return/*€*/1;", "return//€\n1;")
            ):
                observe_native(argv, root / str(index), source.encode())
                self.assertTrue(marker.exists())
                self.assertTrue((root / str(index) / "native/process.json").is_file())
                self.assertFalse((root / str(index) / "policy.json").exists())
                marker.unlink()

    def test_manifest_policy_paths_exclude_native_receipts(self) -> None:
        spec = load_spec(ROOT / "scripts/script-vm-manifest.json")
        paths = set(spec.paths)
        for key, stage in spec.stages:
            if stage == POLICY_STAGE:
                self.assertIn(f"cases/{key}/policy.json", paths)
                self.assertFalse(
                    any(path.startswith(f"cases/{key}/native/") for path in paths)
                )

    def test_missing_policy_receipt_is_rejected(self) -> None:
        source, policy = next(iter(UNDEFINED_INPUTS.items()))
        with TemporaryDirectory() as temporary:
            case = Path(temporary)
            (case / "rust").mkdir()
            _ = (case / "rust/stdout.log").write_text("compile_error\n")
            _ = (case / "rust/stderr.log").write_text(policy.diagnostic())
            write_json(case / "rust/process.json", {"returncode": 0, "expected": 0})
            with self.assertRaises(WorldCheckError):
                compare_case(source, case, POLICY_STAGE)
