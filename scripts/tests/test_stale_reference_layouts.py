import unittest
from pathlib import Path

from scripts.grf_control_evidence import sequence, text
from scripts.world_check_support import at, read_json


class StaleReferenceLayoutsTest(unittest.TestCase):
    def test_guard_layouts_retain_every_copied_reference_file(self) -> None:
        root = Path(__file__).resolve().parents[2]
        expected = {path.name for path in (root / "reference").iterdir() if path.is_file()}
        layouts = {
            "context-ci-layout.json": "stale/stale-source/reference/",
            "language-ci-layout.json": "stale/stale-source/reference/",
            "safety-ci-layout.json": "stale/stale/reference/",
        }
        for name, prefix in layouts.items():
            with self.subTest(layout=name):
                layout = read_json(root / "scripts" / name)
                paths = [text(value) for value in sequence(at(layout, ("guard_paths",)))]
                recorded = {
                    path.removeprefix(prefix)
                    for path in paths
                    if path.startswith(prefix)
                }
                self.assertEqual(recorded, expected)


if __name__ == "__main__":
    _ = unittest.main()
