# SPDX-License-Identifier: GPL-2.0-only
"""Naturally aligned vehicle Random followed by tree placement in one call."""

from __future__ import annotations

from pathlib import Path

from scripts.movement_ci.physical import compare as physical_compare
from scripts.movement_ci.value import field, integer, require
from scripts.tree_growth_mixed import checkpoints, native, require_success, rust
from scripts.tree_growth_run import compare
from scripts.world_check_support import Json, decode_json, read_json, write_json


def coupled_witness(directory: Path) -> None:
    events: list[Json] = []
    with (directory / "trees.jsonl").open() as stream:
        for line in stream:
            event = decode_json(line)
            if field(event, "tick") == 3616:
                events.append(event)
    phases = [(field(event, "phase"), field(event, "edge")) for event in events]
    require(
        "same-call ordered vehicle and placement effects",
        condition=phases
        == [
            ("vehicle_ticks", "enter"),
            ("vehicle_ticks", "leave"),
            ("on_tick_trees", "enter"),
            ("plant_tree", "enter"),
            ("plant_tree", "leave"),
            ("on_tick_trees", "leave"),
        ],
    )
    require(
        "vehicle consumed gameplay Random",
        condition=field(events[0], "random") == [472890882, 840270838]
        and field(events[1], "random") == [779092398, 1132853183],
    )
    require(
        "tree received vehicle-updated Random",
        condition=field(events[2], "random") == field(events[1], "random")
        and field(events[2], "trees_tick_ctr") == 0,
    )
    require(
        "tree underflow consumed next Random",
        condition=field(events[3], "random") == [2937283826, 3318612020]
        and field(events[3], "trees_tick_ctr") == 255,
    )
    require(
        "spontaneous placement preserves low grass density",
        condition=field(events[3], "tile") == 52
        and field(events[3], "raw") == [0, 4, 16, 0, 0, 0, 66, 0, 0, 0]
        and field(events[4], "raw") == [64, 4, 112, 32, 9, 0, 0, 0, 0, 0],
    )
    write_json(directory / "coupled-witness.json", {"call": 37, "events": events})


def _continued(
    original: Path,
    native_lane: Path,
    rust_lane: Path,
    output: Path,
) -> None:
    checkpoints(native_lane, rust_lane, output, 38)
    for call in range(39):
        continued = "initial" if call == 0 else f"tick_{call}"
        uninterrupted = f"tick_{call + 36}"
        for view in ["world", "derived", "content"]:
            compare(
                original / f"{uninterrupted}.{view}.json",
                native_lane / f"{continued}.{view}.json",
                output / f"uninterrupted-{continued}.{view}.json",
            )
        physical_compare(
            read_json(original / f"{uninterrupted}.movement.json"),
            read_json(rust_lane / f"{continued}.physical.json"),
            0,
        )
    write_json(output / "uninterrupted.json", {"all_39_boundaries_equal": True})


def check_mixed(native_binary: Path, runtime: Path, source: Path, output: Path) -> None:
    native(native_binary, source, output / "native", 74)
    require_success(output / "native", 74)
    rust(runtime, source, output / "rust", 74)
    checkpoints(output / "native", output / "rust", output / "compare", 74)
    coupled_witness(output / "native")
    native(native_binary, output / "rust/tick_36.sav", output / "rust-to-native", 38)
    require_success(output / "rust-to-native", 38)
    rust(runtime, output / "native/tick_36.sav", output / "native-to-rust", 38)
    _continued(
        output / "native",
        output / "rust-to-native",
        output / "native-to-rust",
        output / "continued",
    )
    result = read_json(output / "native/results.json")
    require("genuine bounded horizon", condition=integer(field(result, "calls")) == 74)
    write_json(
        output / "PASS.json",
        {
            "uninterrupted_calls": 74,
            "split": 36,
            "remaining": 38,
            "full_world_derived_content_physical": True,
            "same_call_vehicle_random_then_tree_placement": True,
            "both_saved_directions_equal_uninterrupted": True,
        },
    )
