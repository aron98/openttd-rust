"""Table producer inputs must be admitted, not just present on disk."""

import shutil
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
from scripts.world_check_support import WorldCheckError, read_json


@pytest.mark.parametrize(
    "changed",
    [
        "observe.cpp",
        "strings.hpp",
        "constants.hpp",
        "root_slots.hpp",
        "arrays.hpp",
        "tables.hpp",
        "calls.hpp",
        "closures.hpp",
    ],
)
def test_changed_copied_observer_source_is_rejected(
    tmp_path: Path, changed: str
) -> None:
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
        "calls.hpp",
        "closures.hpp",
    ):
        _ = (harness / name).write_text("original observer input")
        _ = (native / name).write_text("original observer input")
    verify_observer_sources(tmp_path, native)
    _ = (native / changed).write_text("substituted observer input")
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
        "crates/ottd-script/src/table/tests/storage.rs",
        "crates/ottd-script/tests/clone-owner.txt",
        "crates/ottd-script/tests/delete-owner.txt",
        "crates/ottd-script/tests/delete-mutation.txt",
        "crates/ottd-script/tests/delete-effects.txt",
        "crates/ottd-script/tests/delete-effects-gnu.txt",
        "crates/ottd-script/tests/table-storage.txt",
        "scripts/compat/script-vm/tables/fixtures/clone-owner.nut",
        "scripts/compat/script-vm/tables/fixtures/delete-owner.nut",
        "scripts/compat/script-vm/tables/fixtures/delete-mutation.nut",
        "scripts/compat/script-vm/tables/fixtures/delete-effects.nut",
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
    assert "native/calls.hpp" in spec.paths
    assert "native/closures.hpp" in spec.paths


@pytest.mark.parametrize(
    "missing",
    [
        "crates/ottd-script/tests/tables-owner.txt",
        "crates/ottd-script/tests/clone-owner.txt",
        "crates/ottd-script/tests/delete-effects.txt",
        "crates/ottd-script/tests/delete-effects-gnu.txt",
        "crates/ottd-script/tests/delete-mutation.txt",
        "crates/ottd-script/tests/delete-owner.txt",
        "crates/ottd-script/tests/table-storage.txt",
        "scripts/compat/script-vm/tables/fixtures/delete-mutation.nut",
        "scripts/compat/script-vm/tables/case.py",
        "scripts/compat/script-vm/tables/objects.py",
        "crates/ottd-script/src/vm/native-calls-callee.txt",
        "scripts/compat/script-vm/calls.hpp",
        "scripts/compat/script-vm/closures.hpp",
        "scripts/compat/script-vm/varargs/case.py",
        "crates/ottd-script/src/vm/closures.rs",
        "scripts/script_vm_roots_spec.py",
        "scripts/script_vm_roots_capture.py",
        "scripts/script_vm_roots_admission.py",
        "scripts/script_vm_arrays_spec.py",
        "scripts/script_vm_arrays_capture.py",
        "scripts/script_vm_arrays_admission.py",
        "scripts/script_vm_arrays_sessions.py",
        "scripts/script_vm_branches_spec.py",
        "scripts/script_vm_branches_capture.py",
    ],
)
def test_missing_embedded_input_rejected_before_native_validation(
    tmp_path: Path,
    missing: str,
) -> None:
    root = Path(__file__).resolve().parents[2]
    spec = load_spec(root / "scripts/script-vm-manifest.json")
    altered = replace(
        spec, sources=tuple(row for row in spec.sources if row[0] != missing)
    )
    with pytest.raises(WorldCheckError, match="VM source closure omits inputs"):
        verify_sources(root, tmp_path, altered)


def test_effects_fixtures_match_both_native_abi_provenance_records() -> None:
    root = Path(__file__).resolve().parents[2]
    provenance = read_json(
        root / "scripts/compat/script-vm/tables/fixtures/delete-effects-provenance.json"
    )
    assert isinstance(provenance, dict)
    source = provenance["source_fixture"]
    assert isinstance(source, str)
    assert digest(root / source) == provenance["source_sha256"]
    observations = provenance["observations"]
    assert isinstance(observations, list)
    observed: set[str] = set()
    for record in observations:
        assert isinstance(record, dict)
        fixture = record["fixture"]
        abi = record["abi"]
        assert isinstance(fixture, str)
        assert isinstance(abi, str)
        assert digest(root / fixture) == record["stdout_sha256"]
        assert record["native_pin"] == PIN
        observed.add(abi)
    assert observed == {"linux-gnu-64-little", "apple-libcxx-abi1-64-little"}


@pytest.mark.parametrize("change", ["missing", "tampered", "symlink"])
def test_closure_observer_copy_must_be_regular_and_identical(
    tmp_path: Path, change: str
) -> None:
    root = Path(__file__).resolve().parents[2]
    native = tmp_path / "native"
    native.mkdir()
    for name in (
        "observe.cpp",
        "strings.hpp",
        "constants.hpp",
        "root_slots.hpp",
        "arrays.hpp",
        "tables.hpp",
        "calls.hpp",
        "closures.hpp",
    ):
        _ = shutil.copy2(root / "scripts/compat/script-vm" / name, native / name)
    verify_observer_sources(root, native)
    target = native / "closures.hpp"
    if change == "tampered":
        _ = target.write_bytes(target.read_bytes() + b"\n// changed\n")
    else:
        target.unlink()
        if change == "symlink":
            target.symlink_to(root / "scripts/compat/script-vm/closures.hpp")
    with pytest.raises(WorldCheckError, match="Native observer source differs"):
        verify_observer_sources(root, native)
