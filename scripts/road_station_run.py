"""Reproduce terminal station construction and two-way original save continuation."""

from __future__ import annotations

import argparse
from pathlib import Path

from scripts.grf_control_evidence import sequence
from scripts.replay_matrix import ReplayMatrix
from scripts.road_station_cases import StationCase, descriptor, portfolio
from scripts.road_station_pair import StationPair
from scripts.road_station_prepare import prepare
from scripts.world_check_support import Json, at, write_json


def request(command: Json) -> Json:
    return {"company": 0, "mode": "post", "command": command}


def composition() -> Json:
    requests: list[Json] = [
        StationCase().request(),
        StationCase(tile=1290, stop_type=1).request(),
    ]
    requests.append(
        request(
            {"kind": "build_road_depot", "tile": 653, "road_type": 0, "direction": 0}
        )
    )
    requests.extend(
        request(
            {
                "kind": "build_vehicle",
                "tile": 653,
                "engine": engine,
                "cargo": 255,
                "use_free_vehicles": False,
                "client_id": 0,
            }
        )
        for engine in (116, 123)
    )
    requests.extend(
        request(
            {
                "kind": "insert_order",
                "location": 0,
                "vehicle": vehicle,
                "index": 0,
                "order": {
                    "type": 33,
                    "flags": 0,
                    "dest": vehicle,
                    "refit_cargo": 254,
                    "wait_time": 0,
                    "travel_time": 0,
                    "max_speed": 65535,
                },
            }
        )
        for vehicle in (0, 1)
    )
    actions: list[Json] = []
    for index, value in enumerate(requests):
        actions.extend(
            (
                {"op": "command", "request": value},
                {"op": "save", "label": f"step-{index}"},
            )
        )
    return {
        "schema_version": 1,
        "case": "construction-purchase-orders",
        "role": "sp",
        "exit": True,
        "actions": actions,
    }


def run(pair: StationPair, source: Path, output: Path) -> None:
    output.mkdir(parents=True, exist_ok=False)
    seed = prepare(
        pair.oracle,
        source,
        output / "seed",
        [request({"kind": "pause", "mode": 0, "paused": True})],
    )
    company = prepare(
        pair.oracle,
        seed,
        output / "prepare-company",
        [
            request({"kind": "road_station_prepare_company"}),
            StationCase(company=1).request(),
        ],
    )
    cash = prepare(
        pair.oracle,
        seed,
        output / "prepare-cash",
        [request({"kind": "road_station_prepare_money", "amount": -1000000})],
    )
    unmodified = prepare(
        pair.oracle,
        seed,
        output / "prepare-unmodified",
        [
            request(
                {
                    "kind": "road_station_prepare_setting",
                    "name": "station.modified_catchment",
                    "value": 0,
                }
            )
        ],
    )
    trees: list[Json] = []
    for tile in range(1940, 1950):
        trees.extend(
            (
                request({"kind": "road_station_prepare_tree", "tile": tile}),
                request({"kind": "landscape_clear", "tile": tile}),
            )
        )
    authority = prepare(pair.oracle, seed, output / "prepare-authority", trees)
    authority = prepare(
        pair.oracle,
        authority,
        output / "prepare-authority-stable",
        [],
        paused_frame=True,
    )
    pair.execute(seed, output / "portfolio", portfolio(), (True,) * 13 + (False,) * 9)
    pair.execute(
        company,
        output / "ownership",
        descriptor(
            (
                StationCase(tile=651, station=0),
                StationCase(tile=651, station=0, company=1),
            )
        ),
        (False, True),
    )
    pair.execute(
        cash,
        output / "cash",
        descriptor((StationCase(), StationCase(estimate=True))),
        (False, True),
    )
    pair.execute(
        unmodified,
        output / "unmodified",
        descriptor(
            (StationCase(tile=130), StationCase(tile=131, station=0, stop_type=1))
        ),
        (True, True),
    )
    pair.execute(
        authority, output / "authority", descriptor((StationCase(tile=2014),)), (False,)
    )
    pair.execute(seed, output / "composition", composition(), (True,) * 7)
    for origin in ("native", "rust"):
        source = output / "portfolio" / origin / "step-0.sav"
        first = output / f"continuation-{origin}-1"
        pair.execute(
            source,
            first,
            descriptor((StationCase(tile=651, station=0, stop_type=1, direction=2),)),
            (True,),
        )
        pair.execute(
            first / "native/step-0.sav",
            output / f"continuation-{origin}-2",
            descriptor((StationCase(tile=652, station=0, direction=1),)),
            (True,),
        )
    cli_case = output / "cli"
    commands = [
        at(action, ("request",))
        for action in sequence(at(composition(), ("actions",)))
        if at(action, ("op",)) == "command"
    ]
    _ = prepare(pair.oracle, seed, cli_case / "native", commands)
    matrix = ReplayMatrix(pair.cli, pair.oracle, pair.oracle, cli_case)
    rust = matrix.rust(seed, cli_case / "native/actions.json", cli_case)
    matrix.compare_outputs(cli_case / "native", rust, cli_case / "comparison")
    write_json(
        output / "PASS.json",
        {
            "paired_commands": 40,
            "cli_commands": 7,
            "cli_checkpoints": 2,
            "boundary_saves": 40,
            "continuation_origins": ["native", "rust"],
        },
    )


class Arguments(argparse.Namespace):
    oracle: Path = Path()
    cli: Path = Path()
    runner: Path = Path()
    source: Path = Path()
    output: Path = Path()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("oracle", "cli", "runner", "source", "output"):
        _ = parser.add_argument(f"--{name}", type=Path, required=True)
    args = Arguments()
    _ = parser.parse_args(namespace=args)
    run(
        StationPair(
            args.oracle.resolve(),
            args.cli.resolve(),
            args.runner.resolve(),
        ),
        args.source.resolve(),
        args.output.resolve(),
    )


if __name__ == "__main__":
    main()
