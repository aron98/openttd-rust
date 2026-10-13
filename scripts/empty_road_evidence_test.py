from __future__ import annotations

from pathlib import Path

import pytest

from scripts.empty_road_controls import archive_controls, roadside_control
from scripts.empty_road_evidence import membership, runtime, saved_anchor
from scripts.empty_road_provenance import admitted, source_names
from scripts.empty_road_run import EmptyRun
from scripts.grf_control_run import ControlRun
from scripts.language_ci_compare import compare
from scripts.world_check_support import Json, WorldCheckError, read_json, write_json


def sample() -> dict[str, Json]:
    return {
        "calendar_date": 767010,
        "calendar_fract": 0,
        "calendar_month": 0,
        "calendar_year": 2100,
        "economy_date": 767010,
        "economy_fract": 0,
        "economy_month": 0,
        "economy_year": 2100,
        "pause": 0,
        "random": [1984111490, 339022207],
        "tick": 0,
        "current_company": 0,
        "interactive_random": [17, 17],
    }


def test_unknown_host_metadata_is_not_silently_removed() -> None:
    value = sample()
    value["native_metadata"] = {"unexpected": 1}
    with pytest.raises(WorldCheckError, match="runtime keys"):
        _ = runtime(value)


@pytest.mark.parametrize("other_bits", [0, 1, 2, 4, 64, 128, 199])
def test_roadside_control_changes_native_grass_to_barren_only(other_bits: int) -> None:
    before = (1 << 3) | other_bits
    after = roadside_control(before)
    assert (after >> 3) & 7 == 0
    assert after & ~0b111000 == before & ~0b111000


def test_only_recognized_runtime_host_fields_are_projected() -> None:
    value = sample()
    expected = {
        k: v
        for k, v in value.items()
        if k not in {"current_company", "interactive_random"}
    }
    assert runtime(value) == expected


@pytest.mark.parametrize("names", [[], ["ticks", "ticks"]])
def test_empty_or_duplicate_roster_never_admits(names: list[str]) -> None:
    with pytest.raises(WorldCheckError, match="membership"):
        membership(names, names)


def test_reordered_roster_is_rejected() -> None:
    with pytest.raises(WorldCheckError):
        membership(["stable", "ticks"], ["ticks", "stable"])


@pytest.mark.parametrize("actual", [["ticks"], ["ticks", "stable", "extra"]])
def test_incomplete_and_extra_case_rosters_refuse(actual: list[str]) -> None:
    with pytest.raises(WorldCheckError):
        membership(actual, ["ticks", "stable"])


@pytest.mark.parametrize("field", ["roadside", "cursor", "random"])
def test_saved_world_fields_never_enter_host_projection(field: str) -> None:
    baseline: Json = {"roadside": 8, "cursor": 1, "random": [1, 2]}
    changed: Json = {"roadside": 8, "cursor": 1, "random": [1, 2]}
    changed[field] = 0
    with pytest.raises(WorldCheckError):
        compare(baseline, changed)


def test_proposed_layout_is_not_ci_admitted(tmp_path: Path) -> None:
    (tmp_path / "scripts").mkdir()
    write_json(
        tmp_path / "scripts/empty-road-layout.json",
        {"fresh_complete_run_verified": False},
    )
    with pytest.raises(WorldCheckError, match="not admitted"):
        _ = admitted(tmp_path, capture_only=False)


def test_integration_embedded_inputs_are_in_source_closure() -> None:
    root = Path(__file__).resolve().parents[1]
    names = source_names(root)
    assert "crates/ottd-sim/tests/road_tile_loop.rs" in names
    assert "fixtures/road-tile/built.sav" in names
    assert "fixtures/road-tile/after-road.world.json" in names
    assert "fixtures/road-tile/cycle-two.derived.json" in names


def test_runtime_rng_is_bound_to_saved_state() -> None:
    world: Json = {
        "chunks": {
            "DATE": {
                "records": {
                    "0": {
                        "tick_counter": 0,
                        "date": 767010,
                        "date_fract": 0,
                        "economy_date": 767010,
                        "economy_date_fract": 0,
                        "pause_mode": 0,
                        "random_state[0]": 1984111490,
                        "random_state[1]": 339022207,
                    }
                }
            }
        }
    }
    saved_anchor(world, sample())
    changed = sample()
    changed["random"] = [0, 0]
    with pytest.raises(WorldCheckError):
        saved_anchor(world, changed)


def test_real_raw_archive_controls_reject_each_independent_corruption(
    tmp_path: Path,
) -> None:
    source = tmp_path / "native/ticks"
    source.mkdir(parents=True)
    write_json(source / "results.json", {"schema_version": 1, "proof": [1, 2, 3]})
    before = (source / "results.json").read_bytes()
    output = tmp_path / "controls"
    output.mkdir()
    run = EmptyRun(ControlRun(tmp_path, tmp_path, tmp_path / "unused-native"), {}, {})
    archive_controls(run, output)
    result = read_json(output / "archive-controls.json")
    assert isinstance(result, dict)
    assert set(result) == {"raw", "archive", "index", "missing", "extra"}
    assert (source / "results.json").read_bytes() == before
