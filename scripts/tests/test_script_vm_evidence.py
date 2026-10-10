"""Admission boundaries for the bounded scalar native matrix."""

import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from scripts.script_vm_evidence import compare_observation, require_paths, require_tests
from scripts.script_vm_provenance import (
    blob_digest,
    check_blob,
    digest,
    load_spec,
    select_executable,
    verify_archive,
)
from scripts.world_check_support import ROOT, WorldCheckError


class ScriptVmEvidenceTests(unittest.TestCase):
    def test_live_workspace_manifest_source_bindings(self) -> None:
        spec = load_spec(ROOT / "scripts/script-vm-manifest.json")
        for name, expected in spec.sources:
            with self.subTest(source=name):
                self.assertEqual(
                    digest(ROOT / name), expected, f"Stale VM source pin: {name}"
                )

    def test_zero_and_subset_tests_are_rejected(self) -> None:
        for output in (
            "test result: ok. 0 passed; 0 failed; 0 ignored;",
            "test one ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;",
        ):
            with self.assertRaises(WorldCheckError):
                require_tests(output, ["one", "two"])

    def test_only_exact_tests_are_admitted(self) -> None:
        require_tests(
            "test one ... ok\ntest two ... ok\ntest result: ok. 2 passed; 0 failed; 0 ignored;",
            ["one", "two"],
        )
        with self.assertRaises(WorldCheckError):
            require_tests(
                "test wrong ... ok\ntest two ... ok\ntest result: ok. 2 passed; 0 failed; 0 ignored;",
                ["one", "two"],
            )

    def test_every_observation_field_is_strict(self) -> None:
        native = "stack 3\nliteral float 2147483648\nop 17 1 2 1 43\nsuspend -1 0\nreturn 99 integer 25\n"
        compare_observation(native, native, "return")
        for old, new in (
            ("stack 3", "stack 4"),
            ("2147483648", "0"),
            ("43", "45"),
            ("-1 0", "0 0"),
            ("-1 0", "-1 1"),
            ("25", "26"),
        ):
            with self.assertRaises(WorldCheckError):
                compare_observation(native, native.replace(old, new), "return")

    def test_equal_empty_and_wrong_stage_observations_are_rejected(self) -> None:
        for text in ("", "compile_error\n"):
            with self.assertRaises(WorldCheckError):
                compare_observation(text, text, "return")

    def test_native_blob_mutation_is_rejected(self) -> None:
        with TemporaryDirectory() as directory:
            path = Path(directory) / "native.cpp"
            _ = path.write_bytes(b"pristine")
            expected = blob_digest(path)
            check_blob(path, expected)
            _ = path.write_bytes(b"altered")
            with self.assertRaises(WorldCheckError):
                check_blob(path, expected)

    def test_cargo_artifact_requires_exact_kind_and_profile(self) -> None:
        with self.assertRaises(WorldCheckError):
            _ = select_executable(
                '{"reason":"compiler-artifact","executable":"wrong","target":{"name":"observe","kind":["bin"]},"profile":{"test":false}}',
                "observe",
                "example",
                False,
            )

    def test_duplicate_cargo_artifacts_are_rejected(self) -> None:
        row = '{"reason":"compiler-artifact","executable":"observer","target":{"name":"observe","kind":["example"]},"profile":{"test":false}}'
        with self.assertRaises(WorldCheckError):
            _ = select_executable(row + "\n" + row, "observe", "example", False)

    def test_packaged_admission_requires_archive_and_index(self) -> None:
        # Given complete raw paths without the required archive artifacts.
        raw = {"summary.json"}
        # When admitting packaged evidence, then missing archive/index fail.
        with self.assertRaises(WorldCheckError):
            require_paths(raw, raw)
        require_paths(raw | {"evidence-index.json", "evidence.tar.gz"}, raw)

    def test_archive_membership_is_verified_independently_of_index_hash(self) -> None:
        import tarfile

        with TemporaryDirectory() as directory:
            root = Path(directory)
            _ = (root / "expected").write_bytes(b"expected")
            _ = (root / "substituted").write_bytes(b"substituted")
            with tarfile.open(root / "evidence.tar.gz", "w:gz") as archive:
                archive.add(root / "substituted", arcname="substituted")
            with self.assertRaises(WorldCheckError):
                verify_archive(root, ("expected",))
