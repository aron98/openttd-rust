"""Discover a complete terminal round trip using only original commands and ticks."""

from __future__ import annotations

import argparse
from pathlib import Path

from scripts.backup_sale_run import CONFIG
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import sequence
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


def commands(engine: int) -> list[Json]:
    result = [request({"kind": "road_route_prepare_acceleration"})]
    result.extend(
        (
            StationCase(tile=650, direction=2, stop_type=int(engine == 123)).request(),
            StationCase(tile=980, direction=3, stop_type=int(engine == 123)).request(),
        )
    )
    roads = {640 + x: 10 for x in range(11, 20)}
    roads[658] = 11
    roads[660] = 12
    roads.update({64 * y + 20: 5 for y in range(11, 15)})
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
    result.extend(
        (
            request(
                {
                    "kind": "build_road_depot",
                    "tile": 594,
                    "road_type": 0,
                    "direction": 1,
                }
            ),
            request(
                {
                    "kind": "build_vehicle",
                    "tile": 594,
                    "engine": engine,
                    "cargo": 255,
                    "use_free_vehicles": False,
                    "client_id": 0,
                }
            ),
        )
    )
    result.extend(
        request(
            {
                "kind": "insert_order",
                "location": 0,
                "vehicle": 0,
                "index": station,
                "order": {
                    "type": 33,
                    "flags": 0,
                    "dest": station,
                    "refit_cargo": 254,
                    "wait_time": 0,
                    "travel_time": 0,
                    "max_speed": 65535,
                },
            }
        )
        for station in range(2)
    )
    result.append(request({"kind": "movement_prepare_start_stop", "vehicle": 0}))
    return result


def discover(oracle: Path, source: Path, output: Path, engine: int) -> None:
    output.mkdir(parents=True, exist_ok=False)
    actions: list[Json] = [
        {"ordinal": i, "op": "command", "request": value}
        for i, value in enumerate(commands(engine))
    ]
    actions.extend(
        (
            {"ordinal": len(actions), "op": "checkpoint", "label": "prepared"},
            {"ordinal": len(actions) + 1, "op": "tick", "count": 5000},
        )
    )
    protocol = output / "actions.json"
    write_json(protocol, {"schema_version": 1, "actions": actions})
    _ = (output / "openttd.cfg").write_text(CONFIG)
    before: Json = {str(p): digest(p) for p in (oracle, source, protocol)}
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
    result = read_json(output / "results.json")
    for action in sequence(at(result, ("actions",))):
        if (
            at(action, ("op",)) == "command"
            and at(action, ("receipt", "posted")) is not True
        ):
            raise WorldCheckError(f"Original route command rejected: {action}")
    after: Json = {str(p): digest(p) for p in (oracle, source, protocol)}
    if before != after:
        raise WorldCheckError("Route discovery inputs changed")
    write_json(output / "bindings-after.json", after)


class Arguments(argparse.Namespace):
    oracle: Path = Path()
    source: Path = Path()
    output: Path = Path()
    engine: int = 116


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("oracle", "source", "output"):
        _ = parser.add_argument(f"--{name}", type=Path, required=True)
    _ = parser.add_argument("--engine", type=int, choices=(116, 123), default=116)
    args = Arguments()
    _ = parser.parse_args(namespace=args)
    discover(
        args.oracle.resolve(), args.source.resolve(), args.output.resolve(), args.engine
    )


if __name__ == "__main__":
    main()
