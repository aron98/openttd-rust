"""Diagnostic native/Rust departure prefixes; never credits a whole route."""

from __future__ import annotations

import argparse
from pathlib import Path

from scripts.backup_sale_run import CONFIG
from scripts.gameplay_foundations import digest
from scripts.road_route_idle import compare
from scripts.world_check_support import Json, WorldCheckError, run, write_json


def pair(oracle: Path, runner: Path, source: Path, output: Path, ticks: int) -> None:
    output.mkdir(parents=True, exist_ok=False)
    native = output / "native"
    native.mkdir()
    _ = (native / "openttd.cfg").write_text(CONFIG)
    write_json(
        native / "actions.json",
        {
            "schema_version": 1,
            "actions": [{"ordinal": 0, "op": "tick", "count": ticks}],
        },
    )
    inputs = (oracle, runner, source, native / "actions.json")
    before: Json = {str(p): digest(p) for p in inputs}
    write_json(output / "bindings-before.json", before)
    _ = run(
        [
            "env",
            "OTTD_ROAD_ROUTE_OBSERVE=1",
            f"OTTD_REPLAY_PATH={native}/actions.json",
            f"OTTD_REPLAY_OUTPUT={native}",
            str(oracle),
            "-X",
            "-x",
            "-c",
            str(native / "openttd.cfg"),
            "-snull",
            "-mnull",
            "-vnull:ticks=8",
            "-g",
            str(source),
        ],
        output / "native-command",
    )
    rust = output / "rust"
    _ = run(
        [
            "env",
            f"ROAD_ROUTE_INPUT={source}",
            f"ROAD_ROUTE_OUTPUT={rust}",
            f"ROAD_ROUTE_TICKS={ticks}",
            str(runner),
            "--exact",
            "native_terminal_round_trip",
            "--ignored",
            "--nocapture",
        ],
        output / "rust-command",
    )
    compare(native, rust, output)
    after: Json = {str(p): digest(p) for p in inputs}
    if before != after:
        raise WorldCheckError("Departure proof inputs changed")
    write_json(output / "bindings-after.json", after)


class Arguments(argparse.Namespace):
    oracle: Path = Path()
    runner: Path = Path()
    source: Path = Path()
    output: Path = Path()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("oracle", "runner", "source", "output"):
        _ = parser.add_argument("--" + name, type=Path, required=True)
    args = Arguments()
    _ = parser.parse_args(namespace=args)
    oracle, runner, source, output = (
        p.resolve() for p in (args.oracle, args.runner, args.source, args.output)
    )
    output.mkdir(parents=True, exist_ok=False)
    for ticks in (1, 16, 32, 64):
        pair(oracle, runner, source, output / str(ticks), ticks)
    for origin in ("native", "rust"):
        pair(
            oracle,
            runner,
            output / "32" / origin / "final.sav",
            output / ("from-" + origin),
            32,
        )
    write_json(
        output / "PREFIX-PASS.json",
        {"paired_prefixes": 6, "whole_route_complete": False},
    )


if __name__ == "__main__":
    main()
