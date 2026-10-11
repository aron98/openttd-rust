"""Table producer inputs must be admitted, not just present on disk."""

from dataclasses import replace
from pathlib import Path

import pytest

from scripts.owned_restore_sources import compiled_inputs, crate_inputs
from scripts.script_vm_inputs import source_names
from scripts.script_vm_provenance import (
    PIN,
    digest,
    load_spec,
    verify_observer_sources,
    verify_sources,
)
from scripts.world_check_support import WorldCheckError


def test_changed_copied_table_header_is_rejected(tmp_path: Path) -> None:
    harness = tmp_path / "scripts/compat/script-vm"
    native = tmp_path / "native"
    harness.mkdir(parents=True)
    (native / "src").mkdir(parents=True)
    _ = (native / "variant.txt").write_text(
        f"pin={PIN}\ninteger_bits=64\nfloat_bits=32\ngc=enabled\n"
    )
    for name in (
        "observe.cpp",
        "strings.hpp",
        "constants.hpp",
        "root_slots.hpp",
        "arrays.hpp",
        "tables.hpp",
    ):
        _ = (harness / name).write_text("original observer input")
        _ = (native / name).write_text("original observer input")
    verify_observer_sources(tmp_path, native)
    _ = (native / "tables.hpp").write_text("substituted table observer")
    with pytest.raises(WorldCheckError, match="Native observer source differs"):
        verify_observer_sources(tmp_path, native)


def test_script_targets_include_embedded_owner_and_mutation_data() -> None:
    root = Path(__file__).resolve().parents[2]
    expected = {
        "crates/ottd-script/src/table/tests.rs",
        "crates/ottd-script/tests/tables-owner.txt",
        "crates/ottd-script/tests/tables-mutation.txt",
        "scripts/compat/script-vm/tables/fixtures/owner.nut",
        "scripts/compat/script-vm/tables/fixtures/mutation.nut",
        "scripts/compat/script-vm/tables/fixtures/null-set.nut",
        "crates/ottd-script/examples/observe.rs",
    }
    assert expected <= set(
        crate_inputs(root, ("ottd-script",), ("src", "tests", "examples"))
    )
    assert expected <= set(source_names(root))
    assert not expected & set(compiled_inputs(root))


def test_active_vm_manifest_closes_current_table_producer_inputs() -> None:
    root = Path(__file__).resolve().parents[2]
    spec = load_spec(root / "scripts/script-vm-manifest.json")
    pinned = dict(spec.sources)
    assert set(source_names(root)) <= set(pinned)
    for name in source_names(root):
        assert pinned[name] == digest(root / name)
        assert "source/" + name in spec.paths
    assert "native/tables.hpp" in spec.paths


def test_missing_embedded_input_rejected_before_native_validation(
    tmp_path: Path,
) -> None:
    root = Path(__file__).resolve().parents[2]
    spec = load_spec(root / "scripts/script-vm-manifest.json")
    missing = "crates/ottd-script/tests/tables-owner.txt"
    altered = replace(
        spec, sources=tuple(row for row in spec.sources if row[0] != missing)
    )
    with pytest.raises(WorldCheckError, match="VM source closure omits inputs"):
        verify_sources(root, tmp_path, altered)
