# /// script
# requires-python = ">=3.12"
# ///
# Run: PYTHONPATH=. python3 -m scripts.road_route_long --help
"""Command-created no-breakdown/service-800 route for authentic graph compression."""

from __future__ import annotations

import argparse
from pathlib import Path

from scripts.backup_sale_run import CONFIG
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import sequence
from scripts.road_route_discover import commands
from scripts.road_route_terminal import Producer, compact
from scripts.road_station_run import request
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    read_json,
    run,
    write_json,
)


class Arguments(argparse.Namespace):
    oracle: Path = Path()
    runner: Path = Path()
    source: Path = Path()
    output: Path = Path()


def prepare(args: Arguments, engine: int, output: Path) -> Path:
    output.mkdir(parents=True, exist_ok=False)
    requests = commands(engine)
    requests.insert(0, request({"kind": "road_route_prepare_breakdowns_disabled"}))
    requests.insert(
        -1,
        request(
            {
                "kind": "change_service_interval",
                "vehicle": 0,
                "interval": 800,
                "custom": True,
                "percent": False,
            }
        ),
    )
    actions: list[Json] = [
        {"ordinal": i, "op": "command", "request": value}
        for i, value in enumerate(requests)
    ]
    actions.append({"ordinal": len(actions), "op": "checkpoint", "label": "prepared"})
    protocol = output / "actions.json"
    write_json(protocol, {"schema_version": 1, "actions": actions})
    _ = (output / "openttd.cfg").write_text(CONFIG)
    before: Json = {
        str(path): digest(path) for path in (args.oracle, args.source, protocol)
    }
    write_json(output / "bindings-before.json", before)
    _ = run(
        [
            "env",
            "OTTD_ROAD_ROUTE_OBSERVE=1",
            "OTTD_MOVEMENT_PREPARE=1",
            f"OTTD_REPLAY_PATH={protocol}",
            f"OTTD_REPLAY_OUTPUT={output}",
            str(args.oracle),
            "-X",
            "-x",
            "-c",
            str(output / "openttd.cfg"),
            "-snull",
            "-mnull",
            "-vnull:ticks=8",
            "-g",
            str(args.source),
        ],
        output / "command",
    )
    for action in sequence(at(read_json(output / "results.json"), ("actions",))):
        if (
            at(action, ("op",)) == "command"
            and at(action, ("receipt", "posted")) is not True
        ):
            raise WorldCheckError(f"Long route original command rejected: {action}")
    after: Json = {
        str(path): digest(path) for path in (args.oracle, args.source, protocol)
    }
    if after != before:
        raise WorldCheckError("Long route command inputs changed")
    write_json(output / "bindings-after.json", after)
    compact(output)
    return output / "prepared.sav"


def prove(args: Arguments) -> None:
    args.output.mkdir(parents=True, exist_ok=False)
    producer = Producer(args.oracle, args.runner)
    for vehicle, engine in (("bus", 116), ("truck", 123)):
        root = args.output / vehicle
        source = prepare(args, engine, root / "commands")
        producer.compare(source, root / "before-compression", 19654)
        for origin in ("native", "rust"):
            producer.compare(
                root / "before-compression" / origin / "final.sav",
                root / ("compression-from-" + origin),
                346,
            )
        producer.compare(source, root / "compression", 20000)
        before = read_json(root / "before-compression" / "native" / "final.world.json")
        after = read_json(root / "compression" / "native" / "final.world.json")
        field = ("chunks", "LGRP", "records", "0", "last_compression")
        if at(before, field) == at(after, field):
            raise WorldCheckError("Native long route did not compress its graph")
        write_json(
            root / "compression-observed.json",
            {"before": at(before, field), "after": at(after, field)},
        )
        for origin in ("native", "rust"):
            producer.compare(
                root / "compression" / origin / "final.sav",
                root / ("from-" + origin),
                1000,
            )
    write_json(
        args.output / "PASS.json",
        {
            "compression_ticks": 20000,
            "continuation_ticks": 1000,
            "save_origins": ["native", "rust"],
        },
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("oracle", "runner", "source", "output"):
        _ = parser.add_argument("--" + name, type=Path, required=True)
    prove(parser.parse_args(namespace=Arguments()))


if __name__ == "__main__":
    main()
