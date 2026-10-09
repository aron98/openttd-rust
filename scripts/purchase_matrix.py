# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 -m scripts.purchase_matrix --oracle ORACLE --artifacts FRESH
"""Prepare an original road fleet and compare live purchase scenarios."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

from scripts.purchase_controls import controls, coverage, resume
from scripts.purchase_replay import scenario
from scripts.replay_matrix import ReplayMatrix
from scripts.world_check_support import (
    ROOT,
    Json,
    WorldCheckError,
    at,
    read_json,
    run,
    write_json,
)


def plan(case: Json) -> Json:
    name = at(case, ("name",))
    requests: list[Json] = []
    for mode, cargo in (
        ("estimate", 255),
        ("post", 255),
        ("post", at(case, ("cargo",))),
        ("post", 254),
        ("post", 64),
    ):
        requests.append(
            {
                "company": 0,
                "mode": mode,
                "command": {
                    "kind": "build_vehicle",
                    "tile": at(case, ("tile",)),
                    "engine": at(case, ("engine",)),
                    "cargo": cargo,
                    "use_free_vehicles": False,
                    "client_id": 0,
                },
            }
        )
    if name == "errors":
        for field, value in (
            ("tile", 0),
            ("engine", 65535),
            ("engine", 0),
            ("tile", 4096),
        ):
            requests.append(
                {
                    "company": 0,
                    "mode": "post",
                    "command": {
                        "kind": "build_vehicle",
                        "tile": at(case, ("tile",)),
                        "engine": at(case, ("engine",)),
                        "cargo": 255,
                        "use_free_vehicles": False,
                        "client_id": 0,
                        field: value,
                    },
                }
            )
        requests.append(
            {
                "company": 14,
                "mode": "post",
                "command": {
                    "kind": "build_vehicle",
                    "tile": at(case, ("tile",)),
                    "engine": at(case, ("engine",)),
                    "cargo": 255,
                    "use_free_vehicles": False,
                    "client_id": 0,
                },
            }
        )
    actions: list[Json] = []
    for request in requests:
        actions.append({"ordinal": len(actions), "op": "command", "request": request})
        actions.append(
            {
                "ordinal": len(actions),
                "op": "checkpoint",
                "label": f"step-{len(actions)}",
            }
        )
    if name in (
        "temperate-original",
        "temperate-realistic",
        "arctic",
        "tropic",
        "toyland",
    ):
        engines = at(case, ("engines",))
        match engines:
            case list():
                pass
            case _:
                raise WorldCheckError("Missing engine coverage")
        for engine in engines:
            actions.append(
                {
                    "ordinal": len(actions),
                    "op": "command",
                    "request": {
                        "company": 0,
                        "mode": "post",
                        "command": {
                            "kind": "build_vehicle",
                            "tile": at(case, ("tile",)),
                            "engine": engine,
                            "cargo": 255,
                            "use_free_vehicles": True,
                            "client_id": 9,
                        },
                    },
                }
            )
    return {"schema_version": 1, "actions": actions}


def execute(matrix: ReplayMatrix) -> None:
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
    inputs = matrix.artifacts / "inputs"
    result = run(
        [
            "env",
            f"PURCHASE_SOURCE={prep}/save/autosave/exit.sav",
            f"PURCHASE_INPUT_DIR={inputs}",
            "cargo",
            "test",
            "--locked",
            "-p",
            "ottd-sim",
            "--test",
            "native_purchase",
            "prepare_purchase_inputs",
            "--",
            "--ignored",
            "--exact",
        ],
        matrix.artifacts / "setup-command",
    )
    if "test result: ok. 1 passed; 0 failed; 0 ignored" not in result.stdout:
        raise WorldCheckError("Purchase preparation did not execute exactly once")
    cases = read_json(inputs / "manifest.json")
    match cases:
        case list():
            pass
        case _:
            raise WorldCheckError("Purchase scenario manifest missing")
    passed: list[Json] = []
    for case in cases:
        name = at(case, ("name",))
        match name:
            case str():
                pass
            case _:
                raise WorldCheckError("Purchase scenario name missing")
        directory = matrix.artifacts / name
        directory.mkdir()
        actions = directory / "actions.json"
        write_json(actions, plan(case))
        scenario(matrix, inputs / f"{name}.sav", actions, directory)
        coverage(case, read_json(directory / "native/results.json"))
        for extension in ("world.json", "derived.json"):
            if name not in ("arctic", "tropic", "toyland"):
                matrix.compare(
                    inputs / f"{name}.{extension}",
                    directory / f"load/native/initial.{extension}",
                    directory / f"setup-{extension}",
                )
        passed.append(name)
        write_json(matrix.artifacts / "progress.json", passed)
        print(f"PASS purchase {name}", flush=True)
    resume(matrix)
    controls(matrix)
    write_json(
        matrix.artifacts / "summary.json",
        {
            "passed": True,
            "cases": passed,
            "negative_controls": 11,
            "command_only_resume": True,
            "climate_inputs_original_canonicalized": True,
        },
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
    matrix = ReplayMatrix(
        args.ottd.resolve(), args.oracle.resolve(), args.oracle.resolve(), artifacts
    )
    write_json(
        artifacts / "provenance.json",
        {
            "native_sha256": hashlib.sha256(matrix.oracle.read_bytes()).hexdigest(),
            "rust_sha256": hashlib.sha256(matrix.cli.read_bytes()).hexdigest(),
            "upstream": "14ec60f248547d4d062a1160f0fc26d742319888",
        },
    )
    execute(matrix)


if __name__ == "__main__":
    main()
