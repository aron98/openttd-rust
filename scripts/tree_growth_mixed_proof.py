# SPDX-License-Identifier: GPL-2.0-only
"""Meaningful tree RNG, vehicle motion and two-way saved continuation proof."""

from __future__ import annotations

from pathlib import Path

from scripts.gameplay_foundations import digest
from scripts.movement_ci.value import array, field, integer, require
from scripts.tree_growth_mixed import checkpoints, native, require_success, rust
from scripts.tree_growth_run import ROOT, compare
from scripts.tree_growth_run import native as replay
from scripts.world_check_support import at, decode_json, read_json, write_json


def witness(output: Path) -> None:
    directory = output / "native"
    events = [
        decode_json(line)
        for line in (directory / "trees.jsonl").read_text().splitlines()
    ]
    tree = [event for event in events if field(event, "phase") == "tile_tree"]
    require("one entered/exited tree callback", condition=len(tree) == 2)
    before, after = tree
    require(
        "natural grown callback",
        condition=field(before, "tile") == 1488
        and field(before, "tick") == 154
        and field(before, "edge") == "enter"
        and field(after, "edge") == "leave"
        and array(field(before, "raw"))[6] == 3,
    )
    require(
        "tree consumes gameplay Random",
        condition=field(before, "random") != field(after, "random"),
    )
    for event in events:
        require(
            "tree counter remains untouched",
            condition=field(event, "trees_tick_ctr") == 225,
        )
    result = read_json(directory / "results.json")
    require(
        "bus crosses two tiles", condition=len(array(field(result, "crossings"))) == 2
    )
    previous = at(
        read_json(directory / "tick_68.world.json"), ("chunks", "DATE", "records", "0")
    )
    daily = at(
        read_json(directory / "tick_69.world.json"), ("chunks", "DATE", "records", "0")
    )
    require(
        "daily boundary crossed",
        condition=integer(field(daily, "date")) == integer(field(previous, "date")) + 1,
    )
    require(
        "daily gameplay RNG changes",
        condition=field(previous, "random_state[0]") != field(daily, "random_state[0]"),
    )
    write_json(
        output / "witness.json",
        {
            "tree_before": before,
            "tree_after": after,
            "vehicle_crossings": field(result, "crossings"),
            "daily_before": previous,
            "daily_after": daily,
            "tree_counter_preserved": True,
        },
    )


def proof(binary: Path, runtime: Path, seed: Path, output: Path) -> None:
    require(
        "authentic movement seed identity",
        condition=digest(seed)
        == "6426fcf323efadc83c3f4220ffea41f2fb74b65ced8304b86630ef2a37022492",
    )
    output.mkdir()
    _ = replay(
        binary,
        seed,
        {"schema_version": 1, "actions": []},
        output / "baseline",
        observe=False,
    )
    prepared = replay(
        binary,
        seed,
        read_json(ROOT / "scripts/tree-growth-mixed-fixture.json"),
        output / "prepare",
        observe=False,
    )
    baseline = read_json(output / "baseline/initial.world.json")
    initial = read_json(output / "prepare/initial.world.json")
    for chunk in ("DATE", "VEHS"):
        require(
            f"fixture preserves complete {chunk}",
            condition=at(baseline, ("chunks", chunk)) == at(initial, ("chunks", chunk)),
        )
    write_json(
        output / "preparation.json",
        {"vehicle_fields_preserved": True, "entire_date_rng_cursor_preserved": True},
    )
    native(binary, prepared, output / "native", 74)
    require_success(output / "native", 74)
    rust(runtime, prepared, output / "rust", 74)
    checkpoints(output / "native", output / "rust", output / "compare", 74)
    witness(output)
    native(binary, output / "rust/tick_37.sav", output / "continued-native", 37)
    require_success(output / "continued-native", 37)
    rust(runtime, output / "native/tick_37.sav", output / "continued-rust", 37)
    checkpoints(
        output / "continued-native",
        output / "continued-rust",
        output / "continued-compare",
        37,
    )
    for family in ("world", "derived", "content"):
        compare(
            output / "native" / f"final.{family}.json",
            output / "continued-native" / f"final.{family}.json",
            output / "continued-compare" / f"uninterrupted.{family}.json",
        )
    native(binary, prepared, output / "observer-off", 74, observe=False)
    result = read_json(output / "observer-off/results.json")
    require(
        "tree admission requires observer opt-in",
        condition=field(result, "calls") == 0
        and field(result, "outcome") == "observer_or_admission_failure",
    )
    write_json(
        output / "PASS.json",
        {
            "full_comparisons": 115,
            "both_way_saved_continuation": True,
            "grown_random_and_vehicle_motion": True,
            "daily_boundary_rng": True,
            "observer_opt_in_required": True,
            "normal_tree_ci_admission": False,
        },
    )
