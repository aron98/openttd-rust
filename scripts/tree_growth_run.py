# SPDX-License-Identifier: GPL-2.0-only
"""Bound native tree replay processes and complete JSON comparisons."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import Final

from scripts.gameplay_foundations import digest
from scripts.grf_control_run import ControlRun
from scripts.language_ci_compare import mapping
from scripts.world_check_support import Json, WorldCheckError, read_json, write_json

ROOT: Final = Path(__file__).resolve().parents[1]


def run(
    command: list[str],
    directory: Path,
    *,
    env: Mapping[str, str] | None = None,
    expected: int = 0,
) -> None:
    job = ControlRun(ROOT, directory, Path(command[0]))
    _ = job.run("process", command, env, expected=expected)


def native(
    binary: Path,
    save: Path,
    protocol: Json,
    directory: Path,
    *,
    observe: bool = True,
) -> Path:
    directory.mkdir(parents=True)
    path = directory / "actions.json"
    write_json(path, protocol)
    env = {"OTTD_REPLAY_PATH": str(path), "OTTD_REPLAY_OUTPUT": str(directory)}
    if observe:
        env["OTTD_TREE_GROWTH_EVENTS"] = str(directory / "events.jsonl")
    write_json(
        directory / "bindings.json",
        {
            "binary": str(binary),
            "binary_sha256": digest(binary),
            "input": str(save),
            "input_sha256": digest(save),
            "protocol_sha256": digest(path),
        },
    )
    run(
        [
            str(binary),
            "-X",
            "-x",
            "-c",
            str(ROOT / "scripts/reference.cfg"),
            "-vnull:ticks=8",
            "-snull",
            "-mnull",
            "-g",
            str(save),
            "-d",
            "sl=2",
        ],
        directory / "process",
        env=env,
    )
    if not (directory / "final.sav").is_file():
        raise WorldCheckError(f"native output missing: {directory}")
    return directory / "final.sav"


def compare(left: Path, right: Path, receipt: Path) -> None:
    if read_json(left) != read_json(right):
        raise WorldCheckError(f"mismatch: {left} {right}")
    write_json(
        receipt,
        {
            "equal": True,
            "expected": str(left),
            "actual": str(right),
            "expected_sha256": digest(left),
            "actual_sha256": digest(right),
        },
    )


def command(kind: str, **values: Json) -> Json:
    return {
        "op": "command",
        "request": {"company": 0, "mode": "post", "command": {"kind": kind, **values}},
    }


def plan(actions: Sequence[Json]) -> Json:
    return {
        "schema_version": 1,
        "actions": [
            {"ordinal": index, **mapping(action)}
            for index, action in enumerate(actions)
        ],
    }
