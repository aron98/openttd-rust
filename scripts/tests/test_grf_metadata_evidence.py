import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from scripts.grf_metadata_evidence import validate_controls
from scripts.world_check_support import WorldCheckError


class MetadataEvidenceTests(unittest.TestCase):
    def test_equal_count_with_wrong_control_identity_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _ = (root / "negative-0-1.json").write_text("{}")
            with self.assertRaises(WorldCheckError):
                _ = validate_controls(root, {"negative-1-1.json"})
            self.assertEqual(len(validate_controls(root, {"negative-0-1.json"})), 1)

    def test_empty_or_escaped_control_cannot_enter_archive(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "results"
            root.mkdir()
            path = root / "negative-0-1.json"
            path.touch()
            with self.assertRaises(WorldCheckError):
                _ = validate_controls(root, {path.name})
            path.unlink()
            outside = root.parent / "control.json"
            _ = outside.write_text("{}")
            path.symlink_to(outside)
            with self.assertRaises(WorldCheckError):
                _ = validate_controls(root, {path.name})
