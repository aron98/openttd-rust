# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 -m scripts.sale_matrix --oracle ORACLE --artifacts FRESH
"""Original sale lifecycle, full state and live group-count acceptance."""

from __future__ import annotations

import argparse
import hashlib
import os
from pathlib import Path

from scripts.replay_matrix import ReplayMatrix
from scripts.sale_cases import buy, commands, plan, sell
from scripts.sale_controls import controls, resume
from scripts.sale_duration_controls import controls as duration_controls
from scripts.sale_replay import scenario
from scripts.world_check_support import (
    ROOT,
    Json,
    WorldCheckError,
    at,
    read_json,
    run,
    write_json,
)


def helper(matrix: ReplayMatrix, test: str, variables: list[str], label: str) -> None:
    result = run(
        [
            "env",
            *variables,
            "cargo",
            "test",
            "--locked",
            "-p",
            "ottd-sim",
            "--test",
            "native_sale",
            test,
            "--",
            "--ignored",
            "--exact",
        ],
        matrix.artifacts / label,
    )
    if "test result: ok. 1 passed; 0 failed; 0 ignored" not in result.stdout:
        raise WorldCheckError("Sale fixture helper did not execute exactly once")


def prepare(matrix: ReplayMatrix) -> Path:
    prep = matrix.artifacts / "prepare"
    _ = run(
        [
            "cmake",
            f"-DORACLE={matrix.oracle}",
            f"-DRUN_DIR={prep}",
            f"-DINPUT={ROOT}/fixtures/replay/clear-v362.sav",
            f"-DCONFIG={ROOT}/scripts/reference.cfg",
            "-DPREPARE=ON",
            "-P",
            str(ROOT / "scripts/check-runtime-road-reference.cmake"),
        ],
        matrix.artifacts / "prepare-command",
    )
    setup = matrix.artifacts / "cleanup-setup"
    helper(
        matrix,
        "cleanup::prepare_sale_cleanup",
        [f"SALE_SOURCE={prep}/save/autosave/exit.sav", f"SALE_SETUP={setup}"],
        "cleanup-helper",
    )
    fleet = read_json(setup / "fleet.json")
    ids = at(fleet, ("ids",))
    tile = at(fleet, ("tile",))
    match ids:
        case list():
            pass
        case _:
            raise WorldCheckError("Original fleet IDs missing")
    cleanup = matrix.artifacts / "cleanup.json"
    write_json(cleanup, commands([(sell(tile, id), "post") for id in ids]))
    cleared = matrix.native(
        setup / "cleanup.sav", cleanup, matrix.artifacts / "original-clear"
    )
    if at(read_json(cleared / "final.world.json"), ("chunks", "VEHS", "records")) != {}:
        raise WorldCheckError("Original cleanup did not sell the entire fleet")
    seed = matrix.artifacts / "seed.json"
    write_json(seed, commands([(buy(tile, at(fleet, ("engine",))), "post")]))
    seeded = matrix.native(
        cleared / "final.sav", seed, matrix.artifacts / "original-seed"
    )
    records = at(read_json(seeded / "final.world.json"), ("chunks", "VEHS", "records"))
    match records:
        case dict() if set(records) == {"0"}:
            pass
        case _:
            raise WorldCheckError("Original purchase did not create seed vehicle0")
    inputs = matrix.artifacts / "inputs"
    paired = matrix.native(
        seeded / "final.sav", seed, matrix.artifacts / "original-pair"
    )
    helper(
        matrix,
        "inputs::prepare_sale_inputs",
        [
            f"SALE_EMPTY={cleared}/final.sav",
            f"SALE_SEED={seeded}/final.sav",
            f"SALE_PAIR={paired}/final.sav",
            f"SALE_INPUTS={inputs}",
        ],
        "inputs-helper",
    )
    return inputs


def coverage(case: Json, results: Json) -> None:
    name = at(case, ("name",))
    actions = at(results, ("actions",))
    match actions:
        case list():
            count = sum(
                at(a, ("receipt", "exec")) is not None
                and at(a, ("receipt", "exec", "success")) is True
                for a in actions
                if at(a, ("op",)) == "command"
            )
        case _:
            raise WorldCheckError("Sale actions missing")
    match name:
        case (
            "temperate-original"
            | "temperate-realistic"
            | "arctic"
            | "tropic"
            | "toyland"
        ):
            engines = at(case, ("engines",))
            match engines:
                case list():
                    expected = 2 * len(engines)
                case _:
                    raise WorldCheckError("Sale engine coverage missing")
        case "reuse":
            expected = 9
        case "legacy":
            expected = 4
        case "named-survivor":
            expected = 3
        case "negative-value" | "minimum-value" | "zero-value" | "zero-location":
            expected = 1
        case (
            "crashed"
            | "moving"
            | "unstopped"
            | "wrong-state"
            | "outside-depot"
            | "nonowner"
            | "pause"
            | "insufficient"
            | "missing"
            | "invalid-location"
        ):
            expected = 0
        case _:
            raise WorldCheckError("Unregistered sale coverage")
    if count != expected:
        raise WorldCheckError(f"Sale coverage {name}: expected {expected}, got {count}")


def execute(matrix: ReplayMatrix) -> None:
    inputs = prepare(matrix)
    cases = read_json(inputs / "manifest.json")
    match cases:
        case list():
            pass
        case _:
            raise WorldCheckError("Sale manifest missing")
    passed: list[Json] = []
    for case in cases:
        name = at(case, ("name",))
        match name:
            case str():
                pass
            case _:
                raise WorldCheckError("Sale name missing")
        directory = matrix.artifacts / name
        directory.mkdir()
        actions = directory / "actions.json"
        write_json(actions, plan(case))
        scenario(matrix, inputs / f"{name}.sav", actions, directory)
        coverage(case, read_json(directory / "native/results.json"))
        passed.append(name)
        write_json(matrix.artifacts / "progress.json", passed)
        print(f"PASS sale {name}", flush=True)
    controls(matrix)
    resume(matrix)
    duration_controls(matrix)
    write_json(
        matrix.artifacts / "summary.json",
        {"passed": True, "cases": passed, "original_cleanup_commands": True},
    )


class Arguments(argparse.Namespace):
    oracle: Path = Path()
    ottd: Path = ROOT / "target/debug/ottd"
    artifacts: Path = Path()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    _ = parser.add_argument("--oracle", type=Path, required=True)
    _ = parser.add_argument("--artifacts", type=Path, required=True)
    _ = parser.add_argument("--ottd", type=Path, default=ROOT / "target/debug/ottd")
    args = Arguments()
    _ = parser.parse_args(namespace=args)
    artifacts = args.artifacts.resolve()
    artifacts.mkdir(parents=True, exist_ok=False)
    os.environ["OTTD_ROAD_SALE_OBSERVE"] = "1"
    matrix = ReplayMatrix(
        args.ottd.resolve(), args.oracle.resolve(), args.oracle.resolve(), artifacts
    )
    write_json(
        artifacts / "provenance.json",
        {
            "native_sha256": hashlib.sha256(matrix.oracle.read_bytes()).hexdigest(),
            "rust_sha256": hashlib.sha256(matrix.cli.read_bytes()).hexdigest(),
            "upstream": "14ec60f248547d4d062a1160f0fc26d742319888",
            "OTTD_ROAD_SALE_OBSERVE": "1",
        },
    )
    execute(matrix)


if __name__ == "__main__":
    main()
