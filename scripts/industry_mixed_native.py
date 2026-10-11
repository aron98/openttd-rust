# SPDX-License-Identifier: GPL-2.0-only
"""Bound passive industry/road observation and native admission receipts."""

from pathlib import Path

from scripts.gameplay_foundations import digest
from scripts.grf_control_run import ControlRun
from scripts.world_check_support import Json, write_json

ROOT = Path(__file__).resolve().parents[1]


def native(
    binary: Path, source: Path, output: Path, calls: int, *, expected: int = 0
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
    config = output / "openttd.cfg"
    _ = config.write_bytes((ROOT / "scripts/reference.cfg").read_bytes())
    variables = {
        "OTTD_MOVEMENT_OBSERVE": "1",
        "OTTD_REPLAY_PATH": str(output / "protocol.json"),
        "OTTD_REPLAY_OUTPUT": str(output),
        "OTTD_TREE_GROWTH_EVENTS": str(output / "trees.jsonl"),
        "OTTD_INDUSTRY_EVENTS": str(output / "industry-events.jsonl"),
        "OTTD_INDUSTRY_CACHE": "1",
        "OTTD_HOUSE_CACHE": "1",
    }
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
    _ = ControlRun(ROOT, output, binary).run(
        "native",
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
        variables,
        expected=expected,
    )
