"""Admission controls for explicit undefined-native-input fixture policy."""

import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from scripts.script_vm_policy import POLICY_STAGE, UNDEFINED_INPUTS, compare_case
from scripts.world_check_support import WorldCheckError, write_json


class ScriptVmPolicyTests(unittest.TestCase):
    def test_only_exact_undefined_bodies_are_classified(self) -> None:
        self.assertEqual(len(UNDEFINED_INPUTS), 8)
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

    def test_policy_requires_exact_typed_rejection_and_keeps_raw_native(self) -> None:
        source = b"return 1;/*x*/\xed\xa0\x80\n"
        diagnostic = (
            "UndefinedNativeCharacter { codepoint: 55296, context: Token } at byte 14\n"
        )
        with TemporaryDirectory() as temporary:
            case = Path(temporary)
            (case / "native").mkdir()
            (case / "rust").mkdir()
            _ = (case / "rust/stdout.log").write_text("compile_error\n")
            _ = (case / "rust/stderr.log").write_text(diagnostic)
            write_json(case / "rust/process.json", {"returncode": 0, "expected": 0})
            for native, status in (
                (b"compile_error\n", 0),
                (b"return 9998 integer 1\n", 0),
                (b"", -11),
            ):
                _ = (case / "native/stdout.log").write_bytes(native)
                write_json(
                    case / "native/process.json",
                    {"returncode": status, "policy": POLICY_STAGE},
                )
                compare_case(source, case, POLICY_STAGE)
                self.assertEqual((case / "native/stdout.log").read_bytes(), native)
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
