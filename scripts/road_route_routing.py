"""Diagnostic native junction/curve/cache consumers with both save origins."""

from __future__ import annotations

import argparse
from pathlib import Path

from scripts.road_route_departure import pair
from scripts.road_route_junction import prepare
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    decode_json,
    read_json,
    write_json,
)


def observed_tie(output: Path) -> None:
    nodes = [
        decode_json(line)
        for line in (output / "73" / "native" / "route-search.jsonl")
        .read_text()
        .splitlines()
    ]
    best: list[Json] = [
        [at(node, (field,)) for field in ("tile", "track", "cost", "estimate")]
        for node in nodes
        if at(node, ("phase",)) == "best_open"
    ]
    if best != [
        [658, 2, 0, 0],
        [658, 4, 0, 0],
        [659, 8, 642, 813],
        [657, 0, 642, 813],
        [786, 3, 1013, 1013],
    ]:
        raise WorldCheckError("Native equal-cost heap expansion order differs")
    write_json(output / "native-best-node-order.json", best)
    first = read_json(output / "73" / "native-last-tick.json")
    if at(first, ("movement", "vehicles", 0, "road", "state")) != 4:
        raise WorldCheckError("Native symmetric tie did not select track 4")
    expected: Json = [{"tile": 786, "trackdir": 3}]
    if at(first, ("movement", "vehicles", 0, "road", "saved_path")) != expected:
        raise WorldCheckError("Native merge choice was not cached in source order")
    last = read_json(output / "400" / "native-last-tick.json")
    if at(last, ("movement", "vehicles", 0, "road", "saved_path")) != []:
        raise WorldCheckError("Native merge choice was not consumed")


class Arguments(argparse.Namespace):
    oracle: Path = Path()
    runner: Path = Path()
    fixtures: Path = Path()
    source: Path = Path()
    output: Path = Path()


def prove(args: Arguments) -> None:
    args.output.mkdir(parents=True, exist_ok=False)
    for name, engine in (("bus", 116), ("truck", 123)):
        ordinary = args.output / (name + "-ordinary")
        source = args.fixtures / (name + "-valid") / "prepared.sav"
        for ticks in (73, 128, 256, 355):
            pair(args.oracle, args.runner, source, ordinary / str(ticks), ticks)
        for origin in ("native", "rust"):
            pair(
                args.oracle,
                args.runner,
                ordinary / "128" / origin / "final.sav",
                ordinary / ("from-" + origin),
                227,
            )
        commands = args.output / (name + "-junction-commands")
        prepare(args.oracle, args.source, commands, engine)
        junction = args.output / (name + "-junction")
        for ticks in (73, 128, 256, 400):
            pair(
                args.oracle,
                args.runner,
                commands / "prepared.sav",
                junction / str(ticks),
                ticks,
            )
        for origin in ("native", "rust"):
            pair(
                args.oracle,
                args.runner,
                junction / "128" / origin / "final.sav",
                junction / ("from-" + origin),
                272,
            )
        observed_tie(junction)
    write_json(
        args.output / "PREFIX-PASS.json",
        {
            "paired_prefixes": 24,
            "full_saved_world_and_road_cache": True,
            "save_origins": ["native", "rust"],
            "whole_route_complete": False,
        },
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("oracle", "runner", "fixtures", "source", "output"):
        _ = parser.add_argument("--" + name, type=Path, required=True)
    args = Arguments()
    _ = parser.parse_args(namespace=args)
    args.oracle = args.oracle.resolve()
    args.runner = args.runner.resolve()
    args.fixtures = args.fixtures.resolve()
    args.source = args.source.resolve()
    args.output = args.output.resolve()
    prove(args)


if __name__ == "__main__":
    main()
