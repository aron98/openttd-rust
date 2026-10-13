"""Prepare witnesses through original Posts and optional paused native continuation."""

from __future__ import annotations

from pathlib import Path

from scripts.backup_sale_run import CONFIG
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import sequence
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    read_json,
    run,
    write_json,
)


def prepare(
    oracle: Path,
    source: Path,
    case: Path,
    requests: list[Json],
    *,
    paused_frame: bool = False,
) -> Path:
    case.mkdir(parents=True, exist_ok=False)
    _ = (case / "openttd.cfg").write_text(CONFIG)
    actions: list[Json] = (
        [{"ordinal": 0, "op": "tick", "count": 1}] if paused_frame else []
    )
    actions.extend(
        {"ordinal": index + int(paused_frame), "op": "command", "request": request}
        for index, request in enumerate(requests)
    )
    plan: Json = {"schema_version": 1, "actions": actions}
    write_json(case / "actions.json", plan)
    before: Json = {
        str(path): digest(path) for path in (oracle, source, case / "actions.json")
    }
    write_json(case / "bindings-before.json", before)
    _ = run(
        [
            "env",
            "OTTD_ROAD_STATION_PREPARE=1",
            f"OTTD_REPLAY_PATH={case}/actions.json",
            f"OTTD_REPLAY_OUTPUT={case}",
            str(oracle),
            "-X",
            "-x",
            "-c",
            str(case / "openttd.cfg"),
            "-snull",
            "-mnull",
            "-vnull:ticks=8",
            "-g",
            str(source),
        ],
        case / "command",
    )
    results = read_json(case / "results.json")
    if paused_frame:
        first = at(results, ("actions", 0))
        if at(first, ("before", "pause")) == 0 or at(first, ("before", "random")) != at(
            first, ("after", "random")
        ):
            raise WorldCheckError(
                "Station stabilization must remain paused without RNG"
            )
    actions = [
        a for a in sequence(at(results, ("actions",))) if at(a, ("op",)) == "command"
    ]
    if len(actions) != len(requests) or any(
        at(a, ("receipt", "posted")) is not True for a in actions
    ):
        raise WorldCheckError("Original station preparation command failed")
    after: Json = {
        str(path): digest(path) for path in (oracle, source, case / "actions.json")
    }
    if before != after:
        raise WorldCheckError("Original station preparation inputs changed")
    write_json(case / "bindings-after.json", after)
    write_json(
        case / "PASS.json",
        {"original_posted_commands": len(actions), "paused_native_frame": paused_frame},
    )
    return case / "final.sav"
