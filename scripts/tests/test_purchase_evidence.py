import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from scripts.grf_metadata_evidence import digest
from scripts.purchase_evidence import paths_from_layout, validate_paths
from scripts.purchase_provenance import binding
from scripts.world_check_support import Json, WorldCheckError


class PurchaseEvidenceTests(unittest.TestCase):
    def test_executed_binary_must_match_recorded_path_and_hash(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            executable = root / "runtime"
            _ = executable.write_bytes(b"recorded executable")
            identity: Json = {"path": str(executable), "sha256": digest(executable)}
            line = f"\x1b[32m Running\x1b[0m unittests src/lib.rs ({executable})\n"
            self.assertEqual(binding(line, identity, root), str(executable.resolve()))
            with self.assertRaises(WorldCheckError):
                _ = binding("", identity, root)
            other = root / "other"
            _ = other.write_bytes(executable.read_bytes())
            with self.assertRaises(WorldCheckError):
                _ = binding(line.replace(str(executable), str(other)), identity, root)
            _ = executable.write_bytes(b"changed executable")
            with self.assertRaises(WorldCheckError):
                _ = binding(line, identity, root)

    def test_missing_case_cannot_be_replaced_by_equal_file_count(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _ = (root / "wrong.json").write_text("{}")
            with self.assertRaises(WorldCheckError):
                _ = validate_paths(root, {"required.json"})
            with self.assertRaises(WorldCheckError):
                _ = paths_from_layout(["required.json", "required.json"])

    def test_empty_proof_and_escaped_file_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "results"
            root.mkdir()
            proof = root / "process.json"
            proof.touch()
            with self.assertRaises(WorldCheckError):
                _ = validate_paths(root, {proof.name})
            proof.unlink()
            outside = root.parent / "outside.json"
            _ = outside.write_text("{}")
            proof.symlink_to(outside)
            with self.assertRaises(WorldCheckError):
                _ = validate_paths(root, {proof.name})

    def test_only_named_process_logs_may_be_empty(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "stdout.log").touch()
            self.assertEqual(len(validate_paths(root, {"stdout.log"})), 1)
            (root / "summary.log").touch()
            with self.assertRaises(WorldCheckError):
                _ = validate_paths(root, {"stdout.log", "summary.log"})
