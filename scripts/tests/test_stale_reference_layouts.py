import unittest
from pathlib import Path

from scripts.grf_control_evidence import sequence, text
from scripts.world_check_support import at, read_json


class StaleReferenceLayoutsTest(unittest.TestCase):
    def test_guard_layouts_retain_every_copied_reference_file(self) -> None:
        root = Path(__file__).resolve().parents[2]
        expected = {
            path.name for path in (root / "reference").iterdir() if path.is_file()
        }
        layouts = {
            "context-ci-layout.json": "stale/stale-source/reference/",
            "language-ci-layout.json": "stale/stale-source/reference/",
            "safety-ci-layout.json": "stale/stale/reference/",
        }
        for name, prefix in layouts.items():
            with self.subTest(layout=name):
                layout = read_json(root / "scripts" / name)
                paths = [
                    text(value) for value in sequence(at(layout, ("guard_paths",)))
                ]
                recorded = {
                    path.removeprefix(prefix)
                    for path in paths
                    if path.startswith(prefix)
                }
                self.assertEqual(recorded, expected)

    def assert_currency_copied_sources(self, paths: list[str]) -> None:
        root = Path(__file__).resolve().parents[2]
        expected = [
            f"stale-source/{directory}/{path.name}"
            for directory in ("scripts", "reference")
            for path in (root / directory).iterdir()
            if path.is_file()
        ]
        copied = [path for path in paths if path.startswith("stale-source/")]
        self.assertCountEqual(copied, expected)

    def test_currency_guard_retains_exact_copied_source_membership(self) -> None:
        root = Path(__file__).resolve().parents[2]
        layout = read_json(root / "scripts/currency-ci-layout.json")
        paths = [
            text(value)
            for value in sequence(at(layout, ("guards", "stale", "paths")))
        ]
        self.assert_currency_copied_sources(paths)

    def test_currency_copied_source_membership_rejects_corruption(self) -> None:
        root = Path(__file__).resolve().parents[2]
        complete = [
            f"stale-source/{directory}/{path.name}"
            for directory in ("scripts", "reference")
            for path in (root / directory).iterdir()
            if path.is_file()
        ]
        self.assert_currency_copied_sources(complete)
        variants = {
            "missing": complete[1:],
            "extra": [*complete, "stale-source/scripts/not-a-real-source.py"],
            "duplicate": [*complete, complete[0]],
            "foreign-directory": [*complete, "stale-source/other/input.py"],
        }
        for name, paths in variants.items():
            with self.subTest(corruption=name), self.assertRaises(AssertionError):
                self.assert_currency_copied_sources(paths)

    def test_string_guard_retains_every_copied_source_file(self) -> None:
        root = Path(__file__).resolve().parents[2]
        layout = read_json(root / "scripts/strings-ci-layout.json")
        paths = [
            text(value)
            for value in sequence(at(layout, ("guards", "stale", "paths")))
        ]
        for directory in ("scripts", "reference"):
            with self.subTest(directory=directory):
                prefix = f"stale-source/{directory}/"
                recorded = {
                    path.removeprefix(prefix)
                    for path in paths
                    if path.startswith(prefix)
                }
                expected = {
                    path.name
                    for path in (root / directory).iterdir()
                    if path.is_file()
                }
                self.assertEqual(recorded, expected)


if __name__ == "__main__":
    _ = unittest.main()
