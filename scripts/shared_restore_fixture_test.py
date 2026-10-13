from copy import deepcopy
from pathlib import Path

import pytest

from scripts.depot_build_archive import package_raw, verify_archive
from scripts.owned_restore_run import RestoreRun
from scripts.purchase_creation import common_path
from scripts.shared_restore_capacity import verify_worlds
from scripts.shared_restore_controls import mutate
from scripts.shared_restore_evidence import validate
from scripts.shared_restore_layout import control_paths, result_paths, source_names
from scripts.shared_restore_strict import compare_saved, creations
from scripts.world_check_support import Json, WorldCheckError, read_json, replace


def test_equal_nonzero_shared_timing_is_not_a_fresh_exception() -> None:
    common: Json = {
        "round_trip_time": 19,
        "depot_unbunching_last_departure": 0,
        "depot_unbunching_next_departure": 0,
    }
    world: Json = {
        "chunks": {"VEHS": {"records": {"0": {"roadveh": [{"common": [common]}]}}}}
    }
    assert common_path("0") == (
        "chunks",
        "VEHS",
        "records",
        "0",
        "roadveh",
        0,
        "common",
        0,
    )
    with pytest.raises(WorldCheckError, match="initialized zero"):
        compare_saved(world, world, (0,))


def test_loaded_nonzero_remains_exact_without_creation_exception() -> None:
    compare_saved({"duration": -99}, {"duration": -99}, ())
    with pytest.raises(WorldCheckError, match="differs"):
        compare_saved({"duration": -99}, {"duration": 0}, ())


def test_empty_roster_does_not_pass(tmp_path: Path) -> None:
    (tmp_path / "results/cases").mkdir(parents=True)
    job = RestoreRun(tmp_path, tmp_path, tmp_path / "native", {}, {"cases": []})
    with pytest.raises(WorldCheckError, match="16-case membership"):
        _ = validate(job)


def test_capacity_requires_exact_existing_and_unrelated_fields() -> None:

    source: Json = {
        "chunks": {"ORDL": {"records": {"7": {"orders": [{"type": 2}]}}}},
        "keep": 19,
    }
    rows: dict[str, Json] = {str(index): {"orders": []} for index in range(64000)}
    rows["7"] = {"orders": [{"type": 2}]}
    prepared: Json = {"chunks": {"ORDL": {"records": rows}}, "keep": 19}
    verify_worlds(source, prepared)
    changed = deepcopy(prepared)

    replace(changed, ("keep",), 20)
    with pytest.raises(WorldCheckError, match="existing/unrelated"):
        verify_worlds(source, changed)


def test_real_archive_detects_raw_corruption(tmp_path: Path) -> None:

    raw = tmp_path / "raw.json"
    _ = raw.write_text('{"strict":true}\n')
    package_raw(tmp_path)
    verify_archive(tmp_path)
    _ = raw.write_text('{"strict":false}\n')
    with pytest.raises(WorldCheckError, match="archived raw evidence changed"):
        verify_archive(tmp_path)


def test_layout_copies_every_source_and_preserves_each_checkpoint() -> None:

    root = Path(__file__).resolve().parents[1]
    sources = source_names(root)
    mutations = read_json(root / "scripts/shared-restore-mutations.json")
    controls = control_paths(mutations, sources)
    assert {
        name.removeprefix("admission/wrong-source/")
        for name in controls
        if name.startswith("admission/wrong-source/")
    } == set(sources)
    assert "fixtures/world/populated-v362.sav" in sources
    assert "crates/ottd-sim/src/runtime/order_state/restore/shared.rs" in sources
    results = result_paths(read_json(root / "scripts/shared-restore-fixtures.json"))
    assert "preparation/capacity-load/original/native/verified.sav" in results
    for name in (
        "two-member",
        "three-member",
        "shared-empty",
        "full-capacity",
        "consume-estimate",
    ):
        assert f"cases/{name}/rust/after.sav" in results
        assert f"cases/{name}/after-compare/process.json" in results


def test_mutation_xor_always_changes_actual_integer() -> None:

    documents: Json = {"actual": {"duration": -2147483648}}
    change: Json = {
        "document": "actual",
        "kind": "xor",
        "path": ["duration"],
        "value": 1,
    }
    assert mutate(documents, change) == -2147483648
    assert documents == {"actual": {"duration": -2147483647}}
    ineffective: Json = {
        "document": "actual",
        "kind": "xor",
        "path": ["duration"],
        "value": 0,
    }
    with pytest.raises(WorldCheckError, match="ineffective"):
        _ = mutate(documents, ineffective)


def test_creation_requires_consumed_clone_and_exact_existing_list() -> None:
    before: Json = {
        "backups": [{"user": 1, "tile": 1560, "clone": 1, "group": 65534}],
        "lists": [{"id": 0, "members": [1]}],
        "vehicles": [{"id": 1}],
        "list_pool": {"items": 1},
    }
    after: Json = {
        "backups": [],
        "lists": [{"id": 0, "members": [1, 0]}],
        "list_pool": {"items": 1},
    }
    returned: Json = {"kind": "vehicle", "vehicle": 0}
    receipt: Json = {
        "posted": True,
        "exec": {"success": True},
        "result": {"success": True},
        "returns": {"exec": returned, "result": returned},
    }
    row: Json = {
        "input": {
            "op": "command",
            "request": {
                "mode": "post",
                "command": {"kind": "build_vehicle", "client_id": 0, "tile": 1560},
            },
        },
        "result": {"receipt": receipt},
        "before": {"orders": before},
        "after": {"orders": after},
    }
    native: Json = {"actions": [row]}
    assert creations(native) == (0,)
    mutations: list[tuple[tuple[str | int, ...], Json, str]] = [
        (("actions", 0, "after", "orders", "list_pool", "items"), 2, "allocated/freed"),
        (
            ("actions", 0, "after", "orders", "lists", 0, "members"),
            [0, 1],
            "immediately after",
        ),
        (("actions", 0, "before", "orders", "backups"), [], "proven shared backup"),
        (("actions", 0, "input", "request", "mode"), "estimate", "committed Post"),
    ]
    for path, value, diagnostic in mutations:
        changed = deepcopy(native)
        replace(changed, path, value)
        with pytest.raises(WorldCheckError, match=diagnostic):
            _ = creations(changed)


def test_legacy_owned_selector_is_still_an_exact_source_test() -> None:
    root = Path(__file__).resolve().parents[1]
    selector = "restore_rejects_matching_clone_group_and_live_non_sp_before_mutation"
    assert selector in (root / "scripts/owned_restore_units.py").read_text()
    assert (
        f"fn {selector}()"
        in (
            root / "crates/ottd-sim/src/runtime/order_state/restore_boundary_tests.rs"
        ).read_text()
    )
