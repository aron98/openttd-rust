# SPDX-License-Identifier: GPL-2.0-only
"""Actual native assertion boundary, kept separate from successful parity saves."""

from __future__ import annotations

import shutil
from pathlib import Path

from scripts.movement_ci.value import field, require
from scripts.tree_growth_proof import Binaries
from scripts.tree_growth_run import ROOT, command, compare, native, plan, run
from scripts.world_check_support import decode_json, write_json


def check_assertion(bins: Binaries, flushed: Path, output: Path) -> None:
    clear = native(
        bins.native,
        ROOT / "fixtures/replay/clear-v362.sav",
        {
            "schema_version": 1,
            "fixture": {"profile": "clear", "paused": False},
            "actions": [],
        },
        output / "clear",
        observe=False,
    )
    planted = native(
        bins.native,
        clear,
        plan(
            [
                command("tree_growth_plant", tile=62, start_tile=1, type=0),
                command("tree_growth_setting", value=2),
            ]
        ),
        output / "planted",
        observe=False,
    )
    prefix = native(
        bins.native, planted, plan([{"op": "tick", "count": 12317}]), output / "prefix"
    )
    run(
        [
            str(bins.cli),
            "replay-world",
            str(planted),
            str(output / "prefix/actions.json"),
            str(output / "rust-prefix"),
        ],
        output / "rust-prefix-command",
    )
    for view in ["world", "derived"]:
        compare(
            output / f"prefix/final.{view}.json",
            output / f"rust-prefix/final.{view}.json",
            output / f"prefix-equal-{view}.json",
        )
    crash = output / "assertion"
    crash.mkdir()
    actions = crash / "actions.json"
    write_json(actions, plan([{"op": "tick", "count": 1}]))
    previous = set((ROOT / "scripts").glob("crash*"))
    run(
        [
            str(flushed),
            "-X",
            "-x",
            "-c",
            str(ROOT / "scripts/reference.cfg"),
            "-vnull:ticks=8",
            "-snull",
            "-mnull",
            "-g",
            str(prefix),
            "-d",
            "sl=2",
        ],
        crash / "process",
        expected=2,
        env={
            "OTTD_REPLAY_PATH": str(actions),
            "OTTD_REPLAY_OUTPUT": str(crash),
            "OTTD_TREE_GROWTH_EVENTS": str(crash / "events.jsonl"),
        },
    )
    for path in set((ROOT / "scripts").glob("crash*")) - previous:
        _ = shutil.move(path, crash / path.name)
    reports = sorted(crash.glob("crash*.json.log"))
    require("original assertion report exists", condition=len(reports) == 1)
    signature = "tile < Map::Size()"
    require("original tile assertion", condition=signature in reports[0].read_text())
    entries = [
        decode_json(line) for line in (crash / "events.jsonl").read_text().splitlines()
    ]
    last = entries[-1]
    require(
        "reached source tile before original assertion",
        condition=field(last, "phase") == "tile_tree"
        and field(last, "edge") == "enter"
        and field(last, "tick") == 12318
        and field(last, "tile") == 29,
    )
    rejected = output / "rejected.json"
    write_json(rejected, plan([{"op": "tick", "count": 12318}]))
    run(
        [
            str(bins.cli),
            "replay-world",
            str(planted),
            str(rejected),
            str(output / "unpublished"),
        ],
        output / "rust-refusal",
        expected=1,
    )
    require(
        "transactional refusal without output",
        condition=not (output / "unpublished").exists()
        and "call 12317: native neighbor tile assertion boundary"
        in (output / "rust-refusal/logs/process/stderr.log").read_text(),
    )
    write_json(
        output / "witness.json",
        {
            "native_exit": 2,
            "signature": signature,
            "last_successful_prefix_calls": 12317,
            "failing_requested_call": 12318,
            "source_tile": 29,
            "last_event": last,
            "crash_save_diagnostic_only": True,
            "assertion_observer_epoch_is_distinct": str(flushed),
            "rust_transactional_refusal": True,
        },
    )
