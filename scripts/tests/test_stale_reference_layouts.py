import unittest
from pathlib import Path

from scripts.currency_ci_capture import source_names
from scripts.grf_control_evidence import sequence, text
from scripts.language_ci_compare import mapping
from scripts.owned_restore_sources import compiled_inputs
from scripts.world_check_support import at, read_json


class StaleReferenceLayoutsTest(unittest.TestCase):
    def assert_complete_source_copy(
        self, sources: set[str], copied: list[str], compiled: set[str]
    ) -> None:
        self.assertTrue(compiled <= sources, sorted(compiled - sources))
        self.assertEqual(copied, sorted(sources))

    def test_all_full_source_rosters_match_actual_compiled_inputs(self) -> None:
        root = Path(__file__).resolve().parents[2]
        compiled = set(compiled_inputs(root))
        layouts = (
            ("order-state-layout.json", "sources", "paths", "source/"),
            ("script-vm-manifest.json", "sources", "paths", "source/"),
            (
                "owned-restore-layout.json",
                "sources",
                "control_paths",
                "admission/wrong-source/",
            ),
            (
                "shared-restore-layout.json",
                "sources",
                "control_paths",
                "admission/wrong-source/",
            ),
            ("empty-road-layout.json", "sources", "artifact_paths", "source/"),
            (
                "empty-road-layout.json",
                "sources",
                "artifact_paths",
                "controls/source/",
            ),
            (
                "tree-terrain/layout.json",
                "source_hashes",
                "artifact_paths",
                "source/",
            ),
        )
        for name, source_key, path_key, prefix in layouts:
            with self.subTest(layout=name, prefix=prefix):
                layout = read_json(root / "scripts" / name)
                sources = set(mapping(at(layout, (source_key,))))
                copied = [
                    text(path).removeprefix(prefix)
                    for path in sequence(at(layout, (path_key,)))
                    if text(path).startswith(prefix)
                ]
                self.assert_complete_source_copy(sources, copied, compiled)

    def test_full_source_copy_rejects_missing_extra_duplicate_and_reordered(
        self,
    ) -> None:
        root = Path(__file__).resolve().parents[2]
        compiled = set(compiled_inputs(root))
        self.assert_complete_source_copy(compiled, sorted(compiled), compiled)
        complete = sorted(compiled)
        variants = {
            "missing": complete[1:],
            "extra": sorted([*complete, "crates/not-a-compiled-input.json"]),
            "duplicate": sorted([*complete, complete[0]]),
            "reordered": list(reversed(complete)),
        }
        for name, copied in variants.items():
            with self.subTest(corruption=name), self.assertRaises(AssertionError):
                self.assert_complete_source_copy(compiled, copied, compiled)

    def test_currency_sources_match_actual_capture_selector(self) -> None:
        root = Path(__file__).resolve().parents[2]
        layout = read_json(root / "scripts/currency-ci-layout.json")
        self.assertCountEqual(mapping(at(layout, ("sources",))), source_names(root))

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
            text(value) for value in sequence(at(layout, ("guards", "stale", "paths")))
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
            text(value) for value in sequence(at(layout, ("guards", "stale", "paths")))
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
                    path.name for path in (root / directory).iterdir() if path.is_file()
                }
                self.assertEqual(recorded, expected)


if __name__ == "__main__":
    _ = unittest.main()
