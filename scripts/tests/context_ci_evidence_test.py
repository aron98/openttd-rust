from copy import deepcopy
from pathlib import Path

import pytest

from scripts.context_ci_evidence import compare_context, validate_context
from scripts.context_ci_support import sources
from scripts.depot_build_archive import package_raw, verify_archive
from scripts.gameplay_foundations import digest
from scripts.occupancy_ci_evidence import compare_occupancy
from scripts.world_check_support import Json, WorldCheckError, replace


def context_pair() -> tuple[Json, Json]:
    clock: dict[str, Json] = dict.fromkeys(
        (
            "calendar_date",
            "calendar_year",
            "calendar_fraction",
            "economy_date",
            "economy_year",
            "economy_fraction",
            "tick",
            "display",
        ),
        0,
    )
    clock.update({"random": [1, 2], "interactive_random": [3, 4]})
    globals_: Json = {"misc": 0, "rail_costs": [8, 12, 16, 24]}
    files: list[Json] = [{"pitch": 0, "width": 29}]
    native: Json = {
        "before": clock,
        "restored": clock,
        "prepared": clock,
        "after_native": clock,
        "files": [None, None, *files],
        "globals": globals_,
    }
    rust: Json = {"clock": clock, "files": files, "globals": globals_}
    return deepcopy(native), deepcopy(rust)


def test_context_numeric_field_cannot_be_boolean() -> None:
    native, rust = context_pair()
    replace(rust, ("globals", "misc"), replacement=False)
    with pytest.raises(WorldCheckError):
        compare_context(native, rust)


def test_context_exact_fixture_is_admitted() -> None:
    native, rust = context_pair()
    compare_context(native, rust)


@pytest.mark.parametrize(
    ("path", "value"),
    [(("globals", "misc"), 1), (("files", 0, "width"), 30), (("clock", "tick"), 1)],
)
def test_context_corrupted_projection_is_rejected(
    path: tuple[str | int, ...], value: Json
) -> None:
    native, rust = context_pair()
    replace(rust, path, value)
    with pytest.raises(WorldCheckError):
        compare_context(native, rust)


def occupancy_pair() -> tuple[Json, Json, Json]:
    live: Json = {"depot": [1], "vehicles": [0]}
    vectors: list[Json] = [
        {
            "vehicle": 0,
            "tile": 42,
            "z": 32,
            "maximum_z": 32,
            "success": False,
            "error_id": 4162,
            "cost": 0,
            "expenses": 255,
        },
        {
            "vehicle": 0,
            "tile": 42,
            "z": 33,
            "maximum_z": 32,
            "success": True,
            "error_id": 65535,
            "cost": 0,
            "expenses": 255,
        },
    ]
    rust: Json = [
        {"success": False, "error_id": 4162, "cost": 0, "expenses": 255},
        {"success": True, "error_id": 65535, "cost": 0, "expenses": 255},
    ]
    canonical: Json = {"runtime": [1], "vehicles": [0]}
    native: Json = {
        "runtime": [1],
        "vehicles": [0],
        "occupancy": {
            "before": deepcopy(live),
            "after": deepcopy(live),
            "hash_restored": {"current": True, "previous": True, "next": True},
            "members_before": [0],
            "members_after": [0],
            "vectors": vectors,
        },
    }
    return native, rust, canonical


def test_occupancy_exact_restored_fixture_is_admitted() -> None:
    native, rust, canonical = occupancy_pair()
    compare_occupancy(native, rust, canonical, canonical)


@pytest.mark.parametrize(
    ("path", "value"),
    [
        (("occupancy", "after", "depot"), [2]),
        (("occupancy", "hash_restored", "current"), False),
        (("occupancy", "members_after"), [1]),
        (("occupancy", "vectors", 1, "z"), 34),
        (("occupancy", "vectors", 1, "vehicle"), 2),
    ],
)
def test_occupancy_corrupted_native_witness_is_rejected(
    path: tuple[str | int, ...], value: Json
) -> None:
    native, rust, canonical = occupancy_pair()
    replace(native, path, value)
    with pytest.raises(WorldCheckError):
        compare_occupancy(native, rust, canonical, canonical)


def test_context_membership_cannot_be_empty(tmp_path: Path) -> None:
    with pytest.raises(WorldCheckError):
        validate_context(tmp_path, {"cases": [], "paths": [], "guard_paths": []})


def test_context_restore_mismatch_is_rejected() -> None:
    with pytest.raises(WorldCheckError):
        compare_context({"before": {"tick": 4}, "restored": {"tick": 5}}, {})


def test_occupancy_missing_live_restoration_is_rejected() -> None:
    with pytest.raises(WorldCheckError):
        compare_occupancy({"occupancy": {"before": 1, "after": 2}}, [], {}, {})


def test_modified_required_source_is_rejected(tmp_path: Path) -> None:
    source = tmp_path / "source.rs"
    _ = source.write_text("fn valid() {}")
    layout: Json = {"sources": {"source.rs": digest(source)}}
    sources(tmp_path, layout)
    _ = source.write_text("fn changed() {}")
    with pytest.raises(WorldCheckError):
        sources(tmp_path, layout)


def test_archive_readback_rejects_changed_raw_proof(tmp_path: Path) -> None:
    proof = tmp_path / "proof.json"
    _ = proof.write_text('{"passed":true}\n')
    package_raw(tmp_path)
    _ = proof.write_text('{"passed":false}\n')
    with pytest.raises(WorldCheckError):
        verify_archive(tmp_path)
