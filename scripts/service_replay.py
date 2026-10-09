# SPDX-License-Identifier: GPL-2.0-only
"""Fresh original road fleet, service command parity, reload, resume and controls."""

from __future__ import annotations

import argparse
import hashlib
from pathlib import Path

from scripts.replay_controls import Mutation, value_control
from scripts.replay_matrix import ReplayMatrix
from scripts.service_replay_cases import CASES, check_native_actions, plan
from scripts.world_check_support import (
    ROOT,
    WorldCheckError,
    at,
    read_json,
    run,
    write_json,
)


def prepare(matrix: ReplayMatrix) -> Path:
    directory = matrix.artifacts / "prepare"
    _ = run(
        [
            "cmake",
            f"-DORACLE={matrix.oracle}",
            f"-DRUN_DIR={directory}",
            f"-DINPUT={ROOT}/fixtures/replay/clear-v362.sav",
            f"-DCONFIG={ROOT}/scripts/reference.cfg",
            "-DPREPARE=ON",
            "-P",
            str(ROOT / "scripts/check-runtime-road-reference.cmake"),
        ],
        matrix.artifacts / "prepare-command",
    )
    inputs = matrix.artifacts / "inputs"
    _ = run(
        [
            "env",
            f"SERVICE_SOURCE={directory}/save/autosave/exit.sav",
            f"SERVICE_INPUT_DIR={inputs}",
            "cargo",
            "test",
            "--locked",
            "-p",
            "ottd-sim",
            "--test",
            "native_service",
            "prepare_service_inputs",
            "--",
            "--ignored",
        ],
        matrix.artifacts / "typed-setup-command",
    )
    return inputs


def resume(matrix: ReplayMatrix, source: Path, actions_path: Path) -> None:
    case = matrix.artifacts / "split"
    case.mkdir()
    actions = at(read_json(actions_path), ("actions",))
    match actions:
        case list():
            write_json(
                case / "prefix.json", {"schema_version": 1, "actions": actions[:4]}
            )
            write_json(
                case / "suffix.json", {"schema_version": 1, "actions": actions[4:]}
            )
        case _:
            raise WorldCheckError("Service plan has no action array")
    native_prefix = matrix.native(source, case / "prefix.json", case / "prefix")
    rust_prefix = case / "prefix/rust"
    _ = run(
        [
            str(matrix.cli),
            "replay-world",
            str(source),
            str(actions_path),
            str(rust_prefix),
            "--through",
            "3",
        ],
        case / "prefix/command",
    )
    matrix.compare_outputs(native_prefix, rust_prefix, case / "prefix/compare")
    native_suffix = matrix.native(
        rust_prefix / "final.sav", case / "suffix.json", case / "suffix"
    )
    rust_suffix = case / "suffix/rust"
    _ = run(
        [
            str(matrix.cli),
            "resume-world",
            str(rust_prefix / "checkpoint.json"),
            str(rust_suffix),
        ],
        case / "suffix/command",
    )
    matrix.compare_outputs(native_suffix, rust_suffix, case / "suffix/compare")
    for extension in ("world.json", "derived.json"):
        matrix.compare(
            matrix.artifacts / f"resume/rust/final.{extension}",
            rust_suffix / f"final.{extension}",
            case / f"continuous-{extension}",
        )


def controls(matrix: ReplayMatrix, vehicle: int) -> None:
    case = matrix.artifacts / "percent"
    for mutation in (
        Mutation(
            "service-cost",
            case / "compare/native-results.json",
            ("actions", 4, "receipt", "test", "cost"),
        ),
        Mutation(
            "service-interval",
            case / "native/final.world.json",
            (
                "chunks",
                "VEHS",
                "records",
                str(vehicle),
                "roadveh",
                0,
                "common",
                0,
                "service_interval",
            ),
        ),
        Mutation(
            "service-flags",
            case / "native/final.world.json",
            (
                "chunks",
                "VEHS",
                "records",
                str(vehicle),
                "roadveh",
                0,
                "common",
                0,
                "vehicle_flags",
            ),
        ),
        Mutation(
            "service-rng",
            case / "native/final.world.json",
            ("chunks", "DATE", "records", "0", "random_state[0]"),
        ),
        Mutation(
            "service-owner-param",
            matrix.artifacts / "nonowner/compare/native-results.json",
            ("actions", 0, "receipt", "result", "error_params", 1),
        ),
    ):
        value_control(matrix, mutation)


def execute(matrix: ReplayMatrix) -> None:
    inputs = prepare(matrix)
    vehicle = at(read_json(inputs / "fleet.json"), ("vehicle",))
    match vehicle:
        case int() if not isinstance(vehicle, bool):
            pass
        case _:
            raise WorldCheckError("Native fleet vehicle ID missing")
    empty = matrix.artifacts / "empty.json"
    write_json(empty, {"schema_version": 1, "actions": []})
    for scenario in CASES:
        case = matrix.artifacts / scenario.name
        case.mkdir()
        actions = case / "actions.json"
        write_json(actions, plan(scenario, vehicle))
        loaded = matrix.native(inputs / f"{scenario.name}.sav", empty, case / "load")
        for extension in ("world.json", "derived.json"):
            matrix.compare(
                inputs / f"{scenario.name}.{extension}",
                loaded / f"initial.{extension}",
                case / f"setup-reload-{extension}",
            )
        source = loaded / "final.sav"
        native = matrix.native(source, actions, case)
        rust = matrix.rust(source, actions, case)
        check_native_actions(read_json(native / "results.json"))
        matrix.compare_outputs(native, rust, case / "compare")
        reloaded = matrix.native(rust / "final.sav", empty, case / "rust-reload")
        for extension in ("world.json", "derived.json"):
            matrix.compare(
                rust / f"final.{extension}",
                reloaded / f"initial.{extension}",
                case / f"rust-reload-{extension}",
            )
        print(f"PASS service {scenario.name}", flush=True)
    resume(
        matrix,
        matrix.artifacts / "resume/load/native/final.sav",
        matrix.artifacts / "resume/actions.json",
    )
    controls(matrix, vehicle)
    write_json(
        matrix.artifacts / "summary.json",
        {
            "passed": True,
            "cases": [case.name for case in CASES],
            "negative_controls": 5,
            "command_only_resume": True,
            "native_reload_every_input_and_rust_output": True,
            "ui_processing": "excluded by stage3 deterministic runner policy; WC_VEHICLE_DETAILS deferred to client integration",
        },
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--oracle", type=Path, default=ROOT / ".reference/snapshot-build/openttd"
    )
    parser.add_argument("--ottd", type=Path, default=ROOT / "target/debug/ottd")
    parser.add_argument("--artifacts", type=Path, required=True)
    args = parser.parse_args()
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
