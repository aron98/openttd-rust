# SPDX-License-Identifier: GPL-2.0-only
"""Bound mixed tree/road observer runs and full runtime comparisons."""

from __future__ import annotations

from pathlib import Path

from scripts.gameplay_foundations import digest
from scripts.movement_ci.physical import compare as physical_compare
from scripts.tree_growth_run import compare, run
from scripts.world_check_support import Json, WorldCheckError, read_json, write_json


def native(
    binary: Path, source: Path, output: Path, calls: int, *, observe: bool = True
) -> None:
    output.mkdir(parents=True)
    protocol: Json = {
        "schema_version": 2,
        "kind": "road_movement",
        "mode": "replay",
        "subject": 0,
        "trace": True,
        "max_calls": 256,
        "max_bytes": 402653184,
        "max_seconds": 60,
        "max_events": 200000,
        "calls": calls,
    }
    write_json(output / "protocol.json", protocol)
    config = Path(__file__).parent / "reference.cfg"
    environment = {
        "OTTD_MOVEMENT_OBSERVE": "1",
        "OTTD_REPLAY_PATH": str(output / "protocol.json"),
        "OTTD_REPLAY_OUTPUT": str(output),
    }
    if observe:
        environment["OTTD_TREE_GROWTH_EVENTS"] = str(output / "trees.jsonl")
    write_json(
        output / "bindings.json",
        {
            "binary": str(binary),
            "binary_sha256": digest(binary),
            "input": str(source),
            "input_sha256": digest(source),
            "config_sha256": digest(config),
            "protocol_sha256": digest(output / "protocol.json"),
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
            "-g",
            str(source),
            "-d",
            "sl=2",
        ],
        output / "process",
        env=environment,
    )


def rust(binary: Path, source: Path, output: Path, calls: int) -> None:
    request = output.with_suffix(".request.json")
    write_json(request, {"input": str(source), "output": str(output), "calls": calls})
    run(
        [str(binary), "--ignored", "--exact", "capture_public_runtime", "--nocapture"],
        output.with_suffix(".process"),
        env={"OTTD_MOVEMENT_RUST_REQUEST": str(request)},
    )


def checkpoints(original: Path, candidate: Path, output: Path, calls: int) -> None:
    output.mkdir()
    for label in ["initial", *(f"tick_{i}" for i in range(1, calls + 1)), "final"]:
        for family in ("world", "derived", "content"):
            compare(
                original / f"{label}.{family}.json",
                candidate / f"{label}.{family}.json",
                output / f"{label}.{family}.json",
            )
        physical_compare(
            read_json(original / f"{label}.movement.json"),
            read_json(candidate / f"{label}.physical.json"),
            0,
        )
        write_json(
            output / f"{label}.physical.json",
            {
                "equal": True,
                "native_sha256": digest(original / f"{label}.movement.json"),
                "rust_sha256": digest(candidate / f"{label}.physical.json"),
            },
        )
    write_json(
        output / "PASS.json",
        {"checkpoints": calls + 2, "full_world_derived_content_physical": True},
    )


def require_success(output: Path, calls: int) -> None:
    result = read_json(output / "results.json")
    if (
        not isinstance(result, dict)
        or result.get("calls") != calls
        or result.get("outcome") != "native_completed"
        or result.get("observer_failure") != "none"
    ):
        raise WorldCheckError(f"Incomplete mixed native capture: {output}")
