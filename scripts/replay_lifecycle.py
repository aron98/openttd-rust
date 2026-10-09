# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 -m scripts.check-replay --artifacts FRESH_PATH
"""Fresh-process replay resume and uninstrumented original continuation witnesses."""

from __future__ import annotations

from scripts.replay_matrix import FIXTURES, ReplayMatrix
from scripts.world_check_support import (
    ROOT,
    WorldCheckError,
    at,
    read_json,
    run,
    write_json,
)


def split_replay(matrix: ReplayMatrix) -> None:
    case = matrix.artifacts / "resume"
    case.mkdir(parents=True)
    actions = at(read_json(FIXTURES / "ticks.json"), ("actions",))
    if not isinstance(actions, list):
        raise WorldCheckError("Replay actions must be a list")
    write_json(case / "prefix.json", {"schema_version": 1, "actions": actions[:4]})
    write_json(case / "suffix.json", {"schema_version": 1, "actions": actions[4:]})
    native_prefix = matrix.native(
        FIXTURES / "clear-v362.sav", case / "prefix.json", case / "prefix"
    )
    rust_prefix = case / "prefix/rust"
    _ = run(
        [
            str(matrix.cli),
            "replay-world",
            str(FIXTURES / "clear-v362.sav"),
            str(FIXTURES / "ticks.json"),
            str(rust_prefix),
            "--through",
            "3",
        ],
        case / "prefix/rust-command",
    )
    matrix.compare_outputs(native_prefix, rust_prefix, case / "prefix/compare")
    native_suffix = matrix.native(
        native_prefix / "final.sav", case / "suffix.json", case / "suffix"
    )
    rust_suffix = case / "suffix/rust"
    _ = run(
        [
            str(matrix.cli),
            "resume-world",
            str(rust_prefix / "checkpoint.json"),
            str(rust_suffix),
        ],
        case / "suffix/rust-command",
    )
    matrix.compare_outputs(native_suffix, rust_suffix, case / "suffix/compare")
    for extension in ("world.json", "derived.json"):
        matrix.compare(
            matrix.artifacts / f"ticks/native/final.{extension}",
            native_suffix / f"final.{extension}",
            case / f"continuous-native-{extension}",
        )
        matrix.compare(
            matrix.artifacts / f"ticks/rust/final.{extension}",
            rust_suffix / f"final.{extension}",
            case / f"continuous-rust-{extension}",
        )


def original_continuation(matrix: ReplayMatrix) -> None:
    case = matrix.artifacts / "original-continuation"
    source = matrix.artifacts / "construction-continue/rust/final.sav"
    output = case / "native"
    _ = run(
        [
            "cmake",
            f"-DORACLE={matrix.original}",
            f"-DRUN_DIR={output}",
            f"-DCONFIG={ROOT}/scripts/reference.cfg",
            f"-DINPUT={source}",
            "-DTICKS=16",
            "-P",
            str(ROOT / "scripts/run-reference.cmake"),
        ],
        case / "command",
    )
    if "[grf:0]" in (output / "stderr.log").read_text():
        raise WorldCheckError("Original continuation reported a content warning")
    saved = output / "save/autosave/exit.sav"
    exported = run(
        [str(matrix.cli), "world", str(saved), "--view", "saved"], case / "world-export"
    )
    world_path = case / "continued.world.json"
    _ = world_path.write_text(exported.stdout)
    before = read_json(matrix.artifacts / "construction-continue/rust/final.world.json")
    after = read_json(world_path)
    tick_path = ("chunks", "DATE", "records", "0", "tick_counter")
    before_tick, after_tick = at(before, tick_path), at(after, tick_path)
    if (
        not isinstance(before_tick, int)
        or not isinstance(after_tick, int)
        or after_tick <= before_tick
    ):
        raise WorldCheckError(
            "Original continuation did not execute an unpaused world tick"
        )
    snapshots = []
    for label, save in [("before", source), ("after", saved)]:
        result = run(
            [str(matrix.cli), "snapshot", str(save)], case / f"{label}-snapshot"
        )
        path = case / f"{label}.snapshot.json"
        _ = path.write_text(result.stdout)
        snapshots.append(read_json(path))
    before_tile = at(snapshots[0], ("map", "tiles", 3184))
    after_tile = at(snapshots[1], ("map", "tiles", 3184))
    if before_tile != after_tile:
        raise WorldCheckError(
            "Original continuation did not preserve constructed road tile"
        )
    write_json(
        case / "witness.json",
        {
            "original_binary": str(matrix.original),
            "before_tick": before_tick,
            "after_tick": after_tick,
            "road_tile": 3184,
            "road_preserved": True,
            "scope": "Original load and actual continuation; no Rust parity claim for populated gameplay ticks",
        },
    )
