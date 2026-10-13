# SPDX-License-Identifier: GPL-2.0-only
"""Original commands, generated maps and natural waiting for spreading witnesses."""

from __future__ import annotations

import shutil
from pathlib import Path

from scripts.gameplay_foundations import digest
from scripts.movement_ci.value import field, require
from scripts.tree_growth_run import ROOT, command, native, plan, run
from scripts.world_check_support import at, read_json, write_json


def setting(binary: Path, seed: Path, output: Path, value: int) -> Path:
    result = native(
        binary,
        seed,
        plan([command("tree_growth_setting", value=value)]),
        output,
        observe=False,
    )
    receipt = read_json(output / "results.json")
    require(
        "original setting command succeeded",
        condition=at(receipt, ("actions", 0, "receipt", "posted")) is True
        and at(receipt, ("actions", 0, "receipt", "result", "success")) is True,
    )
    require(
        "original setting saved",
        condition=at(
            read_json(output / "final.world.json"),
            ("chunks", "PATS", "records", "0", "construction.extra_tree_placement"),
        )
        == value,
    )
    write_json(
        output / "seed-binding.json",
        {
            "input": str(seed),
            "sha256": digest(seed),
            "setting": value,
            "original_receipt": receipt,
        },
    )
    return result


def large(binary: Path, output: Path) -> Path:
    output.mkdir()
    config = output / "openttd.cfg"
    _ = config.write_text(
        (ROOT / "scripts/reference.cfg")
        .read_text()
        .replace("map_x = 6", "map_x = 9")
        .replace("map_y = 6", "map_y = 9")
        .replace("starting_year = 1950", "starting_year = 2100")
    )
    write_json(
        output / "actions.json",
        {
            "schema_version": 1,
            "fixture": {"profile": "clear", "paused": False},
            "actions": [],
        },
    )
    run(
        [
            str(binary),
            "-X",
            "-x",
            "-c",
            str(config),
            "-vnull:ticks=8",
            "-snull",
            "-mnull",
            "-t",
            "2100",
            "-G",
            "12345",
            "-g",
            "-d",
            "sl=2",
        ],
        output / "process",
        env={
            "OTTD_REPLAY_PATH": str(output / "actions.json"),
            "OTTD_REPLAY_OUTPUT": str(output),
        },
    )
    return output / "final.sav"


def moving(binary: Path, seed: Path, output: Path) -> Path:
    waited = native(
        binary, seed, plan([{"op": "tick", "count": 3426}]), output / "wait"
    )
    directory = output / "prepare"
    directory.mkdir()
    _ = shutil.copy2(
        ROOT / "scripts/tree-spread-road-prepare.json", directory / "protocol.json"
    )
    _ = shutil.copy2(ROOT / "scripts/reference.cfg", directory / "openttd.cfg")
    run(
        [
            str(binary),
            "-X",
            "-x",
            "-c",
            str(directory / "openttd.cfg"),
            "-vnull:ticks=8",
            "-snull",
            "-mnull",
            "-g",
            str(waited),
            "-d",
            "sl=2",
        ],
        directory / "process",
        env={
            "OTTD_MOVEMENT_OBSERVE": "1",
            "OTTD_MOVEMENT_PREPARE": "1",
            "OTTD_REPLAY_PATH": str(directory / "protocol.json"),
            "OTTD_REPLAY_OUTPUT": str(directory),
            "OTTD_TREE_GROWTH_EVENTS": str(directory / "trees.jsonl"),
        },
    )
    result = read_json(directory / "results.json")
    require(
        "actual original moving witness",
        condition=field(result, "outcome") == "native_moving_witness"
        and field(result, "calls") == 153
        and field(result, "observer_failure") == "none",
    )
    return directory / "tick_153.sav"
