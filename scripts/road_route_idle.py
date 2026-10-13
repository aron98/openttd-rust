"""Compare full idle-route World and road caches through both save origins."""

from __future__ import annotations

import argparse
from pathlib import Path

from scripts.backup_sale_run import CONFIG
from scripts.world_check_support import (
    WorldCheckError,
    at,
    read_json,
    run,
    write_json,
)


def native(oracle: Path, source: Path, output: Path) -> None:
    output.mkdir(parents=True, exist_ok=False)
    _ = (output / "openttd.cfg").write_text(CONFIG)
    write_json(
        output / "actions.json",
        {"schema_version": 1, "actions": [{"ordinal": 0, "op": "tick", "count": 1000}]},
    )
    _ = run(
        [
            "env",
            "OTTD_ROAD_ROUTE_OBSERVE=1",
            f"OTTD_REPLAY_PATH={output}/actions.json",
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


def rust(runner: Path, source: Path, output: Path, ticks: int = 1000) -> None:
    _ = run(
        [
            "env",
            f"ROAD_ROUTE_INPUT={source}",
            f"ROAD_ROUTE_OUTPUT={output}",
            f"ROAD_ROUTE_TICKS={ticks}",
            str(runner),
            "--exact",
            "native_idle_station_phase",
            "--ignored",
            "--nocapture",
        ],
        output.parent / (output.name + "-command"),
    )


def compare(left: Path, right: Path, output: Path) -> None:
    if read_json(left / "final.world.json") != read_json(right / "final.world.json"):
        raise WorldCheckError("Idle route full World differs")
    lines = (left / "route.jsonl").read_text().splitlines()
    # Preserve the raw native line for the shared JSON parser.
    _ = (output / "native-last-tick.json").write_text(lines[-1] + "\n")
    native_state = read_json(output / "native-last-tick.json")
    rust_state = read_json(right / "final.runtime.json")
    if at(native_state, ("movement", "runtime_road", "road")) != at(
        rust_state, ("road",)
    ):
        raise WorldCheckError("Idle route road caches differ")
    write_json(output / "PASS.json", {"full_saved_world": True, "road_caches": True})


def prove(oracle: Path, runner: Path, source: Path, output: Path) -> None:
    output.mkdir(parents=True, exist_ok=False)
    native(oracle, source, output / "native")
    rust(runner, source, output / "rust")
    compare(output / "native", output / "rust", output)
    for origin in ("native", "rust"):
        continuation = output / ("from-" + origin)
        continuation.mkdir()
        source = output / origin / "final.sav"
        native(oracle, source, continuation / "native")
        rust(runner, source, continuation / "rust")
        compare(continuation / "native", continuation / "rust", continuation)
    write_json(
        output / "continuation-PASS.json",
        {"origins": ["native", "rust"], "ticks_per_segment": 1000},
    )


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
    prove(
        args.oracle.resolve(),
        args.runner.resolve(),
        args.source.resolve(),
        args.output.resolve(),
    )


if __name__ == "__main__":
    main()
