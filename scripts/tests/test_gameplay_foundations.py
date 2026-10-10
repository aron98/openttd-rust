import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from scripts.gameplay_foundations import fresh_directory, require_files, require_test
from scripts.world_check_support import WorldCheckError


class FoundationAdmissionTests(unittest.TestCase):
    def test_reused_or_external_directory_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            requested = root / ".artifacts/fresh"
            self.assertEqual(fresh_directory(root, str(requested)), requested)
            with self.assertRaises(FileExistsError):
                fresh_directory(root, str(requested))
            with self.assertRaises(WorldCheckError):
                fresh_directory(root, str(root / "outside"))

    def test_zero_or_ignored_tests_cannot_report_success(self):
        name = "native_probe"
        require_test(
            f"test {name} ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 filtered out;",
            name,
        )
        for output in [
            "test result: ok. 0 passed; 0 failed; 1 ignored;",
            "test another_probe ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;",
        ]:
            with self.assertRaises(WorldCheckError):
                require_test(output, name)

    def test_missing_empty_and_escaped_evidence_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "proof"
            root.mkdir()
            with self.assertRaises(WorldCheckError):
                require_files(root, ["missing.json"])
            (root / "empty.json").touch()
            with self.assertRaises(WorldCheckError):
                require_files(root, ["empty.json"])
            outside = root.parent / "outside.json"
            outside.write_text('{"passed":true}')
            (root / "escaped.json").symlink_to(outside)
            with self.assertRaises(WorldCheckError):
                require_files(root, ["escaped.json"])
            with self.assertRaises(WorldCheckError):
                require_files(root, ["../outside.json"])
