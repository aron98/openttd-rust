"""Pinned evidence lists preserve the effective contract and failure behavior."""

import hashlib
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))

from contract_layouts import DRIVER_FIELDS, expand_artifact_lists
from contract_model import (
    Contract,
    ContractError,
    Driver,
    Json,
    array,
    decode_json,
    read_json,
    record,
    strings,
)
from contract_run import run_driver
from contract_validate import load_contract


def declaration(data: bytes, path: str = "layout.json") -> dict[str, Json]:
    return {
        "assets": [
            {
                "id": "layout",
                "path": path,
                "sha256": hashlib.sha256(data).hexdigest(),
                "kind": "evidence-layout",
                "provenance": "test",
                "save_version": None,
                "profile": None,
            }
        ],
        "drivers": [
            {
                "id": "test",
                "argv": ["python3"],
                "artifact_root": "outputs",
                "artifact_globs": {"asset": "layout"},
                "success_text": "PASS",
                "scopes": ["rust_behavior"],
                "timeout_seconds": 2,
            }
        ],
    }


class ContractLayoutTests(unittest.TestCase):
    def test_terrain_artifact_roster_is_sorted_and_unique(self) -> None:
        layout = record(
            decode_json((ROOT / "scripts/tree-terrain/layout.json").read_bytes()),
            "fresh_complete_run_verified source_hashes native_membership "
            "rust_membership artifact_paths",
        )
        paths = strings(layout["artifact_paths"])
        self.assertEqual(paths, tuple(sorted(set(paths))))

    def test_repository_json_source_pins_include_nested_layouts(self) -> None:
        checked: set[tuple[str, str]] = set()
        paths: list[Path] = []
        for directory, children, files in os.walk(ROOT / "scripts"):
            children[:] = [name for name in children if not name.startswith(".")]
            paths.extend(
                Path(directory) / name
                for name in files
                if name.endswith(".json") and not name.startswith(".")
            )
        for path in sorted(paths):
            relative = path.relative_to(ROOT)
            pending: list[tuple[str, Json]] = [("", decode_json(path.read_bytes()))]
            while pending:
                location, value = pending.pop()
                if isinstance(value, dict):
                    for field, child in value.items():
                        label = f"{location}/{field}"
                        if field in ("sources", "source_hashes") and isinstance(
                            child, dict
                        ):
                            checked.add((relative.as_posix(), field))
                            for name, expected in child.items():
                                with self.subTest(
                                    layout=str(relative), field=label, source=name
                                ):
                                    self.assertEqual(
                                        hashlib.sha256(
                                            (ROOT / name).read_bytes()
                                        ).hexdigest(),
                                        expected,
                                    )
                        pending.extend(
                            [(label, child)] if isinstance(child, (dict, list)) else []
                        )
                elif isinstance(value, list):
                    pending.extend(
                        (f"{location}/{index}", child)
                        for index, child in enumerate(value)
                    )
        self.assertIn(("scripts/script-vm-manifest.json", "sources"), checked)
        self.assertIn(("scripts/tree-terrain/layout.json", "source_hashes"), checked)

    def test_external_array_preserves_order_and_multiplicity(self) -> None:
        # Given an ordered pinned literal array, including a repeated requirement.
        data = b'["second.txt", "first.txt", "second.txt"]'
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _ = (root / "layout.json").write_bytes(data)
            raw = declaration(data)
            before = json.dumps(raw)
            # When resolving the reference, then only its representation changes.
            expanded, verified = expand_artifact_lists(raw, root)
            self.assertEqual(
                record(array(expanded["drivers"])[0], DRIVER_FIELDS)["artifact_globs"],
                ["second.txt", "first.txt", "second.txt"],
            )
            self.assertEqual(verified["layout"], data)
            self.assertEqual(json.dumps(raw), before)

    def test_bad_sha_fails_before_parsing_json(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _ = (root / "layout.json").write_bytes(b"not JSON")
            with self.assertRaisesRegex(ContractError, "sha256"):
                _ = expand_artifact_lists(declaration(b"[]"), root)

    def test_invalid_literal_roots_and_elements_are_rejected(self) -> None:
        for data in (
            b'{"asset":"layout"}',
            b"[3]",
            b'[""]',
            b'["  "]',
            b'[{"asset":"layout"}]',
            b'[{"x":1,"x":2}]',
            b"not JSON",
            b'"text"',
            b"\xff",
            b"[" * 2000,
        ):
            with (
                self.subTest(data=data[:30]),
                tempfile.TemporaryDirectory() as directory,
            ):
                root = Path(directory)
                _ = (root / "layout.json").write_bytes(data)
                with self.assertRaises(ContractError):
                    _ = expand_artifact_lists(declaration(data), root)

    def test_oversize_and_repeated_reference_budget_are_rejected(self) -> None:
        for count, size in ((1, 1024 * 1024 + 1), (3, 512 * 1024)):
            with self.subTest(count=count), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                data = b'["proof"]' + b" " * (size - len(b'["proof"]'))
                _ = (root / "layout.json").write_bytes(data)
                raw = declaration(data)
                raw["drivers"] = array(raw["drivers"]) * count
                with self.assertRaisesRegex(ContractError, "byte limit"):
                    _ = expand_artifact_lists(raw, root)

    def test_layout_paths_and_all_symlink_components_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            data = b'["proof"]'
            (root / "real").mkdir()
            _ = (root / "real/layout.json").write_bytes(data)
            (root / "link").symlink_to(root / "real", target_is_directory=True)
            (root / "file.json").symlink_to(root / "real/layout.json")
            for path in (
                "../layout.json",
                str(root / "real/layout.json"),
                "link/layout.json",
                "file.json",
                "missing.json",
            ):
                with self.subTest(path=path), self.assertRaises(ContractError):
                    _ = expand_artifact_lists(declaration(data, path), root)

    def test_reference_identity_and_metadata_are_strict(self) -> None:
        data = b'["proof"]'
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _ = (root / "layout.json").write_bytes(data)
            references: tuple[Json, ...] = (
                {"asset": "missing"},
                {"asset": "layout", "path": "layout.json"},
                {"asset": 1},
                None,
            )
            for reference in references:
                raw = declaration(data)
                record(array(raw["drivers"])[0], DRIVER_FIELDS)["artifact_globs"] = (
                    reference
                )
                with (
                    self.subTest(reference=reference),
                    self.assertRaises(ContractError),
                ):
                    _ = expand_artifact_lists(raw, root)
            for field, value in (
                ("kind", "recipe"),
                ("profile", "profile"),
                ("save_version", 1),
            ):
                raw = declaration(data)
                record(
                    array(raw["assets"])[0],
                    "id path sha256 kind provenance save_version profile",
                )[field] = value
                with self.subTest(field=field), self.assertRaises(ContractError):
                    _ = expand_artifact_lists(raw, root)
            raw = declaration(data)
            raw["assets"] = array(raw["assets"]) * 2
            with self.assertRaisesRegex(ContractError, "ambiguous"):
                _ = expand_artifact_lists(raw, root)

    def test_duplicate_reference_key_is_rejected_at_json_boundary(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "input.json"
            _ = path.write_text('{"asset":"one","asset":"two"}')
            with self.assertRaisesRegex(ContractError, "duplicate"):
                _ = read_json(path)

    def test_exact_cumulative_limit_is_accepted(self) -> None:
        data = b'["proof"]' + b" " * (512 * 1024 - len(b'["proof"]'))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _ = (root / "layout.json").write_bytes(data)
            raw = declaration(data)
            raw["drivers"] = array(raw["drivers"]) * 2
            expanded, _ = expand_artifact_lists(raw, root)
            self.assertEqual(len(array(expanded["drivers"])), 2)

    def test_outside_symlink_is_rejected_even_with_correct_sha(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            parent = Path(directory)
            root = parent / "repo"
            root.mkdir()
            data = b'["proof"]'
            _ = (parent / "outside.json").write_bytes(data)
            (root / "layout.json").symlink_to(parent / "outside.json")
            with self.assertRaisesRegex(ContractError, "symlink"):
                _ = expand_artifact_lists(declaration(data), root)

    def test_real_producer_admission_is_representation_independent(self) -> None:
        for mode, expected in (
            ("success", True),
            ("missing", False),
            ("failure", False),
        ):
            outcomes: list[tuple[bool, int | None, str | None]] = []
            drivers: list[Driver] = []
            with tempfile.TemporaryDirectory() as directory:
                for external in (False, True):
                    root = Path(directory) / str(external)
                    contract = probe_contract(root, external, mode)
                    driver = contract.drivers[0]
                    result = run_driver(driver, root, root / "run")
                    outcomes.append((result.passed, result.exit_code, result.error))
                    drivers.append(driver)
                    self.assertEqual(result.passed, expected)
            self.assertEqual(drivers[0], drivers[1])
            self.assertEqual(outcomes[0], outcomes[1])

    def test_scenario_evidence_uses_expanded_requirements(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory) / "repo"
            _ = probe_contract(root, True, "success")
            path = root / "contract.json"
            raw = record(
                read_json(path),
                "schema_version target assets profiles drivers scenarios",
            )
            scenario = record(
                array(raw["scenarios"])[0],
                "id status scope domain expected driver assets source_refs evidence",
            )
            scenario["evidence"] = ["not-declared.txt"]
            _ = path.write_text(json.dumps(raw))
            with self.assertRaisesRegex(ContractError, "evidence not required"):
                _ = load_contract(path, root)


def probe_contract(root: Path, external: bool, mode: str) -> Contract:
    root.mkdir(parents=True)
    (root / "scripts").mkdir()
    (root / "outputs").mkdir()
    for name in ("upstream.toml", "scripts/setup-reference.sh"):
        _ = (root / name).write_bytes((ROOT / name).read_bytes())
    _ = (root / "source.txt").write_text("producer evidence")
    raw = declaration(b'["proof.txt","second.txt"]')
    raw["schema_version"] = 1
    original = record(
        read_json(ROOT / "compatibility/contract.json"),
        "schema_version target assets profiles drivers scenarios",
    )
    raw["target"] = original["target"]
    source: Json = {
        "id": "source",
        "path": "source.txt",
        "sha256": hashlib.sha256((root / "source.txt").read_bytes()).hexdigest(),
        "kind": "recipe",
        "provenance": "real producer",
        "save_version": None,
        "profile": None,
    }
    raw["assets"] = [source, *array(raw["assets"])] if external else [source]
    raw["profiles"] = [
        {
            "id": "vanilla",
            "settings": ["test"],
            "settings_assets": ["source"],
            "content_metadata": None,
            "content": [],
        }
    ]
    code = "from pathlib import Path; p=Path('outputs/new'); p.mkdir(); (p/'proof.txt').write_text('proof'); "
    if mode == "success":
        code += "(p/'second.txt').write_text('second'); "
    code += "print('Artifacts: '+str(p.resolve())); print('PASS')"
    if mode == "failure":
        code += "; raise SystemExit(9)"
    driver = record(array(raw["drivers"])[0], DRIVER_FIELDS)
    driver["argv"] = [sys.executable, "-c", code]
    if external:
        _ = (root / "layout.json").write_bytes(b'["proof.txt","second.txt"]')
    else:
        driver["artifact_globs"] = ["proof.txt", "second.txt"]
    raw["scenarios"] = [
        {
            "id": "probe",
            "status": "implemented",
            "scope": "rust_behavior",
            "domain": "representation",
            "expected": "exact artifacts",
            "driver": "test",
            "assets": ["source"],
            "source_refs": ["source.txt"],
            "evidence": ["proof.txt", "second.txt"],
        }
    ]
    _ = (root / "contract.json").write_text(json.dumps(raw))
    return load_contract(root / "contract.json", root)
