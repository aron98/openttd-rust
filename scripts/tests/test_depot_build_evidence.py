import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from scripts.depot_build_archive import bounded_paths, package_raw, verify_archive
from scripts.depot_build_evidence import strings, validate_cases
from scripts.depot_build_provenance import binding
from scripts.gameplay_foundations import digest, require_test
from scripts.world_check_support import Json, WorldCheckError, write_json


class DepotBuildEvidenceTests(unittest.TestCase):
    def test_missing_case_cannot_be_replaced_by_equal_path_count(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            _ = (root / "wrong.json").write_text("{}")
            with self.assertRaises(WorldCheckError):
                _ = bounded_paths(root, {"required.json"})
            with self.assertRaises(WorldCheckError):
                _ = strings(["same", "same"])

    def test_empty_substantive_file_and_symlink_directory_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            path = root / "process.json"
            path.touch()
            with self.assertRaises(WorldCheckError):
                _ = bounded_paths(root, {path.name})
            path.unlink()
            (root / "loop").symlink_to(root, target_is_directory=True)
            with self.assertRaises(WorldCheckError):
                _ = bounded_paths(root, set())

    def test_only_named_stdout_stderr_files_may_be_empty(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            (root / "stdout.log").touch()
            self.assertEqual(len(bounded_paths(root, {"stdout.log"})), 1)
            (root / "comparison.log").touch()
            with self.assertRaises(WorldCheckError):
                _ = bounded_paths(root, {"stdout.log", "comparison.log"})

    def test_subset_summary_is_rejected_before_claiming_full_matrix(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            (root / "inputs").mkdir()
            write_json(root / "inputs/manifest.json", {"cases": ["subset"]})
            layout: Json = {"cases": [f"case-{i}" for i in range(178)]}
            with self.assertRaises(WorldCheckError):
                _ = validate_cases(root, layout)

    def test_zero_tests_and_wrong_test_name_are_rejected(self) -> None:
        with self.assertRaises(WorldCheckError):
            require_test("test result: ok. 0 passed; 0 failed; 0 ignored;", "expected")
        with self.assertRaises(WorldCheckError):
            require_test(
                "test other ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;",
                "expected",
            )

    def test_executed_binary_requires_path_hash_kind_and_test_profile(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            original, retained = root / "original", root / "retained"
            for path in (original, retained):
                _ = path.write_bytes(b"real binary identity fixture")
            identity: Json = {
                "original": str(original),
                "retained": str(retained),
                "sha256": digest(original),
                "kind": ["lib"],
                "profile_test": True,
            }
            line = f"\x1b[32m Running\x1b[0m unittests src/lib.rs ({original})\n"
            self.assertEqual(binding(line, identity, root), str(original))
            for wrong in ("", line + line, line.replace(str(original), str(retained))):
                with self.assertRaises(WorldCheckError):
                    _ = binding(wrong, identity, root)
            mutations: tuple[tuple[str, Json], ...] = (
                ("kind", ["rlib"]),
                ("profile_test", False),
            )
            for key, value in mutations:
                wrong_identity: Json = {
                    "original": str(original),
                    "retained": str(retained),
                    "sha256": digest(original),
                    "kind": ["lib"],
                    "profile_test": True,
                }
                wrong_identity[key] = value
                with self.assertRaises(WorldCheckError):
                    _ = binding(line, wrong_identity, root)
            _ = retained.write_bytes(b"changed")
            with self.assertRaises(WorldCheckError):
                _ = binding(line, identity, root)

    def test_archive_verification_rejects_raw_and_archive_corruption(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            raw = root / "proof.json"
            _ = raw.write_bytes(b'{"real": "test input"}')
            package_raw(root)
            original = raw.read_bytes()
            _ = raw.write_bytes(b"corrupt")
            with self.assertRaises(WorldCheckError):
                verify_archive(root)
            _ = raw.write_bytes(original)
            with (root / "evidence.tar.gz").open("ab") as archive:
                _ = archive.write(b"changed")
            with self.assertRaises(WorldCheckError):
                verify_archive(root)
