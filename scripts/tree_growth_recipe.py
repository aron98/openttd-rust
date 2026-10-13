# SPDX-License-Identifier: GPL-2.0-only
"""Original-command preparation and adversarial tree proof controls."""

from __future__ import annotations

import json
import shutil
from pathlib import Path

from scripts.gameplay_foundations import digest
from scripts.language_ci_compare import mapping
from scripts.tree_growth_proof import Binaries
from scripts.tree_growth_run import ROOT, command, compare, native, plan, run
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    decode_json,
    read_json,
    write_json,
)


def prepare(bins: Binaries, output: Path) -> Path:
    fixture: Json = {
        "profile": "clear",
        "paused": False,
        "terrain_growth": True,
        "date": {"year": 2100, "month": 0, "day": 1, "fract": 0},
        "tick": 0,
        "trees": {
            "tiles": [
                {
                    "tile": (10 + stage) * 64 + 10 + count,
                    "type": stage,
                    "count": count,
                    "ground": (count - 1) % 2,
                    "stage": stage,
                    "density": (count - 1) % 4,
                }
                for stage in range(7)
                for count in range(1, 5)
            ],
            "towns": [],
            "threshold": 20,
            "magic_bulldozer": False,
        },
    }
    seed = native(
        bins.native,
        ROOT / "fixtures/replay/clear-v362.sav",
        {"schema_version": 1, "fixture": fixture, "actions": []},
        output / "prepare-fixture",
        observe=False,
    )
    prepared = native(
        bins.native,
        seed,
        plan(
            [
                command(
                    "tree_growth_plant",
                    tile=32 * 64 + 32,
                    start_tile=27 * 64 + 27,
                    type=0,
                )
            ]
        ),
        output / "prepare-command",
        observe=False,
    )
    receipt = at(
        read_json(output / "prepare-command/results.json"), ("actions", 0, "receipt")
    )
    if (
        at(receipt, ("posted",)) is not True
        or at(receipt, ("result", "success")) is not True
    ):
        raise WorldCheckError(f"original plant command failed: {receipt}")
    return prepared


def corrupt_cursor(bins: Binaries, output: Path) -> None:
    corrupt = output / "corrupt-cursor"
    corrupt.mkdir()
    _ = shutil.copy2(output / "no-spread/native/initial.sav", corrupt / "initial.sav")
    with (output / "no-spread/native/events.jsonl").open() as stream:
        first = mapping(decode_json(stream.readline()))
    if first["phase"] != "vehicle_ticks" or first["edge"] != "enter":
        raise WorldCheckError("cursor corruption witness expects first vehicle entry")
    first["cursor"] = 0
    # A single JSON object is also a one-line JSONL input after compact encoding.
    _ = (corrupt / "events.jsonl").write_text(json.dumps(first) + "\n")
    run(
        [
            str(bins.test),
            "--ignored",
            "--exact",
            "native_tree_events_match_complete_world_ticks",
            "--nocapture",
        ],
        corrupt / "command",
        env={"OTTD_TREE_NATIVE_DIR": str(corrupt)},
        expected=101,
    )
    if (corrupt / "rust-event-proof.json").exists():
        raise WorldCheckError("corrupt event produced successful proof")
    write_json(corrupt / "rejected.json", {"actual_cursor_mutation_rejected": True})


def pause_and_zero(bins: Binaries, prepared: Path, output: Path) -> None:
    paused = native(
        bins.native,
        prepared,
        plan([command("pause", mode=0, paused=True)]),
        output / "prepare-paused",
        observe=False,
    )
    for name, source, count in [("zero", prepared, 0), ("paused", paused, 8)]:
        directory = output / name
        _ = native(
            bins.native,
            source,
            plan([{"op": "tick", "count": count}]),
            directory / "native",
        )
        run(
            [
                str(bins.cli),
                "replay-world",
                str(source),
                str(directory / "native/actions.json"),
                str(directory / "rust"),
            ],
            directory / "rust-command",
        )
        for view in ["world", "derived"]:
            compare(
                directory / f"native/final.{view}.json",
                directory / f"rust/final.{view}.json",
                directory / f"equal-{view}.json",
            )
        if (directory / "native/events.jsonl").exists():
            raise WorldCheckError(f"paused/zero call emitted gameplay callback: {name}")


def late_failure(bins: Binaries, prepared: Path, output: Path) -> None:
    failure = output / "late-failure"
    failure.mkdir()
    edit = failure / "edit.json"
    write_json(
        edit,
        {
            "schema_version": 1,
            "edits": [
                {
                    "kind": "field",
                    "chunk": "DATE",
                    "record": 0,
                    "path": ["competitors_interval"],
                    "value": {"unsigned": 2},
                }
            ],
        },
    )
    source = failure / "input.sav"
    run(
        [str(bins.cli), "edit-world", str(prepared), str(edit), str(source)],
        failure / "prepare",
    )
    protocol = failure / "actions.json"
    write_json(protocol, plan([{"op": "tick", "count": 2}]))
    before = digest(source)
    run(
        [
            str(bins.cli),
            "replay-world",
            str(source),
            str(protocol),
            str(failure / "rejected-output"),
        ],
        failure / "command",
        expected=1,
    )
    stderr = failure / "command/logs/process/stderr.log"
    if (
        source.exists()
        and digest(source) == before
        and not (failure / "rejected-output").exists()
        and "tick_timer" in stderr.read_text()
    ):
        write_json(
            failure / "rollback.json",
            {
                "late_tick_failure": True,
                "input_unchanged": True,
                "output_unpublished": True,
            },
        )
    else:
        raise WorldCheckError("late-failure publication/rollback control failed")
