# SPDX-License-Identifier: GPL-2.0-only
"""Full state-loop spreading comparisons and bidirectional saved continuation."""

from __future__ import annotations

from pathlib import Path

from scripts.movement_ci.value import array, field, integer, require
from scripts.tree_growth_proof import Binaries
from scripts.tree_growth_run import compare, native, plan, run
from scripts.world_check_support import read_json, write_json


def check_case(bins: Binaries, seed: Path, directory: Path, ticks: int) -> None:
    midpoint = ticks // 2
    protocol = plan(
        [
            {"op": "tick", "count": midpoint},
            {"op": "checkpoint", "label": "mid"},
            {"op": "tick", "count": ticks - midpoint},
        ]
    )
    _ = native(bins.native, seed, protocol, directory / "native")
    run(
        [
            str(bins.cli),
            "replay-world",
            str(seed),
            str(directory / "native/actions.json"),
            str(directory / "rust"),
        ],
        directory / "rust-command",
    )
    for label in ["initial", "mid", "final"]:
        for view in ["world", "derived"]:
            compare(
                directory / f"native/{label}.{view}.json",
                directory / f"rust/{label}.{view}.json",
                directory / f"equal-{label}-{view}.json",
            )
    run(
        [
            str(bins.test),
            "--ignored",
            "--exact",
            "native_spreading_events_match_complete_world_ticks",
            "--nocapture",
        ],
        directory / "event-test",
        env={"OTTD_TREE_SPREAD_NATIVE_DIR": str(directory / "native")},
    )
    proof = read_json(directory / "native/rust-spread-proof.json")
    require(
        "complete native event proof",
        condition=field(proof, "calls") == ticks
        and field(proof, "whole_world_equal") is True
        and field(proof, "derived_equal") is True,
    )
    _ = native(
        bins.native,
        directory / "rust/mid.sav",
        plan([{"op": "tick", "count": ticks - midpoint}]),
        directory / "rust-to-native",
    )
    run(
        [
            str(bins.cli),
            "replay-world",
            str(directory / "native/mid.sav"),
            str(directory / "rust-to-native/actions.json"),
            str(directory / "native-to-rust"),
        ],
        directory / "native-to-rust-command",
    )
    for lane in ["rust-to-native", "native-to-rust"]:
        for view in ["world", "derived"]:
            compare(
                directory / f"native/final.{view}.json",
                directory / f"{lane}/final.{view}.json",
                directory / f"equal-{lane}-{view}.json",
            )
    _ = native(bins.native, seed, protocol, directory / "observer-off", observe=False)
    for label in ["initial", "mid", "final"]:
        for view in ["world", "derived"]:
            compare(
                directory / f"native/{label}.{view}.json",
                directory / f"observer-off/{label}.{view}.json",
                directory / f"equal-observer-off-{label}-{view}.json",
            )


def lifecycle(directory: Path) -> None:
    proof = read_json(directory / "native/rust-spread-proof.json")
    branches = array(field(proof, "grown_branches"))
    require(
        "every Grown branch",
        condition=len(branches) == 8 and all(integer(value) > 0 for value in branches),
    )
    for name in [
        "saturated_branch_one",
        "count_additions",
        "count_removals",
        "plants",
        "clears",
        "post_vehicle_plants",
    ]:
        require(f"native lifecycle {name}", condition=integer(field(proof, name)) > 0)
    require(
        "neighbor and post-vehicle placements",
        condition=integer(field(proof, "plants"))
        > integer(field(proof, "post_vehicle_plants")),
    )
    write_json(directory / "lifecycle.json", {"native_lifecycle_coverage": proof})
