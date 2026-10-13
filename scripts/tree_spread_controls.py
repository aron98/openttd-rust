# SPDX-License-Identifier: GPL-2.0-only
"""Adversarial controls against the actual complete-loop event comparator."""

from __future__ import annotations

import argparse
import json
import shutil
from pathlib import Path

from scripts.language_ci_compare import mapping
from scripts.movement_ci.value import field, integer, require
from scripts.tree_growth_run import run
from scripts.world_check_support import Json, at, decode_json, read_json, write_json


def reject_corrupt_counter(binary: Path, native: Path, output: Path) -> None:
    output.mkdir(parents=True, exist_ok=False)
    _ = shutil.copy2(native / "initial.sav", output / "initial.sav")
    entries: list[Json] = []
    with (native / "events.jsonl").open() as stream:
        for line in stream:
            event = mapping(decode_json(line))
            entries.append(event)
            if (
                field(event, "phase") == "on_tick_trees"
                and field(event, "edge") == "leave"
            ):
                event["trees_tick_ctr"] = integer(field(event, "trees_tick_ctr")) ^ 1
                break
    require("found complete original call", condition=bool(entries))
    _ = (output / "events.jsonl").write_text(
        "".join(json.dumps(entry) + "\n" for entry in entries)
    )
    run(
        [
            str(binary),
            "--ignored",
            "--exact",
            "native_spreading_events_match_complete_world_ticks",
            "--nocapture",
        ],
        output / "command",
        env={"OTTD_TREE_SPREAD_NATIVE_DIR": str(output)},
        expected=101,
    )
    require(
        "counter discrepancy rejected before publication",
        condition=not (output / "rust-spread-proof.json").exists()
        and "assertion `left == right` failed"
        in (output / "command/logs/process/stderr.log").read_text(),
    )
    write_json(output / "rejected.json", {"changed_saved_tree_counter_rejected": True})


def motion_and_daily(directory: Path) -> None:
    result = read_json(directory / "native/results.json")
    require(
        "two actual vehicle tile crossings",
        condition=field(result, "crossings")
        == [
            {"call": 37, "from": 670, "to": 669},
            {"call": 74, "from": 669, "to": 668},
        ],
    )
    before = at(
        read_json(directory / "native/tick_46.world.json"),
        ("chunks", "DATE", "records", "0"),
    )
    after = at(
        read_json(directory / "native/tick_47.world.json"),
        ("chunks", "DATE", "records", "0"),
    )
    require(
        "natural daily boundary within coupled horizon",
        condition=field(before, "date") == 767058
        and field(before, "date_fract") == 73
        and field(after, "date") == 767059
        and field(after, "date_fract") == 0
        and field(after, "tick_counter") == 3626,
    )
    require(
        "daily gameplay Random effects",
        condition=[field(before, "random_state[0]"), field(before, "random_state[1]")]
        == [2937283826, 3318612020]
        and [field(after, "random_state[0]"), field(after, "random_state[1]")]
        == [1187207583, 1440902301],
    )
    require(
        "complete state and physical comparisons exist",
        condition=field(
            read_json(directory / "compare/PASS.json"),
            "full_world_derived_content_physical",
        )
        is True,
    )
    write_json(
        directory / "motion-and-daily.json",
        {
            "crossings": field(result, "crossings"),
            "daily_before": before,
            "daily_after": after,
            "asserted_actual_effects": True,
        },
    )


class Arguments(argparse.Namespace):
    test: Path = Path()
    native: Path = Path()
    output: Path = Path()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["test", "native", "output"]:
        _ = parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args(namespace=Arguments())
    reject_corrupt_counter(
        args.test.resolve(), args.native.resolve(), args.output.resolve()
    )


if __name__ == "__main__":
    main()
