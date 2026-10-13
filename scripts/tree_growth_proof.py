# SPDX-License-Identifier: GPL-2.0-only
"""Complete loop, continuation and native lifecycle evidence for tree growth."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from scripts.movement_ci.value import array, field, integer
from scripts.tree_growth_run import compare, native, plan, run
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    decode_json,
    read_json,
    write_json,
)


@dataclass(frozen=True, slots=True)
class Binaries:
    native: Path
    cli: Path
    test: Path


def check_case(bins: Binaries, seed: Path, directory: Path, ticks: int) -> None:
    protocol = plan(
        [
            {"op": "tick", "count": ticks // 2},
            {"op": "checkpoint", "label": "mid"},
            {"op": "tick", "count": ticks - ticks // 2},
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
            "native_tree_events_match_complete_world_ticks",
            "--nocapture",
        ],
        directory / "event-test",
        env={"OTTD_TREE_NATIVE_DIR": str(directory / "native")},
    )
    proof = read_json(directory / "native/rust-event-proof.json")
    if (
        at(proof, ("calls",)) != ticks
        or integer(at(proof, ("callbacks",))) <= 0
        or at(proof, ("whole_world_equal",)) is not True
        or at(proof, ("derived_equal",)) is not True
    ):
        raise WorldCheckError("native event test produced incomplete proof")
    remaining = plan([{"op": "tick", "count": ticks - ticks // 2}])
    _ = native(
        bins.native, directory / "rust/mid.sav", remaining, directory / "rust-to-native"
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
    entries: dict[int, Json] = {}
    transitions: dict[str, list[tuple[int, int]]] = {}
    branches: set[int] = set()
    grass = 0
    with (directory / "native/events.jsonl").open() as stream:
        for line in stream:
            event = decode_json(line)
            if field(event, "phase") != "tile_tree":
                continue
            tile = integer(field(event, "tile"))
            if field(event, "edge") == "enter":
                entries[tile] = event
                continue
            before = entries.pop(tile)
            before_raw = array(field(before, "raw"))
            after_raw = array(field(event, "raw"))
            pair = integer(before_raw[6]) & 7, integer(after_raw[6]) & 7
            if pair[0] != pair[1]:
                transitions.setdefault(str(tile), []).append(pair)
            if field(before, "random") != field(event, "random"):
                branches.add(integer(at(event, ("random", 1))) & 7)
            if before_raw[3] != after_raw[3]:
                grass += 1
    complete = [
        tile
        for tile, changes in transitions.items()
        if 27 <= int(tile) // 64 <= 32
        and 27 <= int(tile) % 64 <= 32
        and changes[:7] == [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 0)]
    ]
    if not complete or branches != set(range(8)) or not grass:
        raise WorldCheckError(
            "native corpus missed a planted lifecycle, RNG branch, or grass update"
        )
    encoded: dict[str, Json] = {
        tile: [[start, end] for start, end in changes]
        for tile, changes in transitions.items()
    }
    completed_tiles: list[Json] = list(complete)
    observed_branches: list[Json] = []
    observed_branches.extend(sorted(branches))
    write_json(
        directory / "lifecycle.json",
        {
            "complete_original_planted_lifecycles": completed_tiles,
            "all_grown_random_branches": observed_branches,
            "grass_changes": grass,
            "transitions": encoded,
        },
    )
