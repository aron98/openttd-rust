"""Genuine command-created symmetric junction and saved YAPF cache witness."""

from __future__ import annotations

import argparse
from pathlib import Path

from scripts.backup_sale_run import CONFIG
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import sequence
from scripts.road_route_discover import commands
from scripts.road_station_cases import StationCase
from scripts.road_station_run import request
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    read_json,
    run,
    write_json,
)


def junction_commands(engine: int) -> list[Json]:
    original = commands(engine)
    result = [original[0]]
    result.extend(
        StationCase(tile=tile, direction=3, stop_type=int(engine == 123)).request()
        for tile in (914, 980)
    )
    roads = {
        658: 11,
        657: 10,
        656: 6,
        720: 5,
        784: 3,
        785: 10,
        786: 14,
        787: 10,
        788: 9,
        724: 5,
        660: 12,
        659: 10,
        850: 5,
    }
    result.extend(
        request(
            {
                "kind": "build_road",
                "tile": tile,
                "pieces": bits,
                "road_type": 0,
                "toggle_disallowed": 0,
                "town_id": 65535,
            }
        )
        for tile, bits in roads.items()
    )
    result.extend(original[-5:])
    return result


def prepare(oracle: Path, source: Path, output: Path, engine: int) -> None:
    output.mkdir(parents=True, exist_ok=False)
    actions: list[Json] = [
        {"ordinal": i, "op": "command", "request": value}
        for i, value in enumerate(junction_commands(engine))
    ]
    actions.extend(
        (
            {"ordinal": len(actions), "op": "checkpoint", "label": "prepared"},
            {"ordinal": len(actions) + 1, "op": "tick", "count": 400},
        )
    )
    protocol = output / "actions.json"
    write_json(protocol, {"schema_version": 1, "actions": actions})
    _ = (output / "openttd.cfg").write_text(CONFIG)
    inputs = (oracle, source, protocol)
    before: Json = {str(p): digest(p) for p in inputs}
    write_json(output / "bindings-before.json", before)
    _ = run(
        [
            "env",
            "OTTD_ROAD_ROUTE_OBSERVE=1",
            "OTTD_MOVEMENT_PREPARE=1",
            f"OTTD_REPLAY_PATH={protocol}",
            f"OTTD_REPLAY_OUTPUT={output}",
            str(oracle),
            "-X",
            "-x",
            "-c",
            str(output / "openttd.cfg"),
            "-snull",
            "-mnull",
            "-vnull:ticks=8",
            "-g",
            str(source),
        ],
        output / "command",
    )
    for action in sequence(at(read_json(output / "results.json"), ("actions",))):
        if at(action, ("op",)) == "command" and (
            at(action, ("receipt", "posted")) is not True
            or any(
                at(action, ("receipt", phase, "success")) is not True
                for phase in ("test", "exec", "result")
            )
        ):
            raise WorldCheckError(f"Original junction command rejected: {action}")
    after: Json = {str(p): digest(p) for p in inputs}
    if before != after:
        raise WorldCheckError("Junction proof inputs changed")
    write_json(output / "bindings-after.json", after)


class Arguments(argparse.Namespace):
    oracle: Path = Path()
    source: Path = Path()
    output: Path = Path()
    engine: int = 116


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("oracle", "source", "output"):
        _ = parser.add_argument("--" + name, type=Path, required=True)
    _ = parser.add_argument("--engine", type=int, choices=(116, 123), default=116)
    args = Arguments()
    _ = parser.parse_args(namespace=args)
    prepare(
        args.oracle.resolve(), args.source.resolve(), args.output.resolve(), args.engine
    )


if __name__ == "__main__":
    main()
