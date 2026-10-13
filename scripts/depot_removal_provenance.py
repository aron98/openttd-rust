from __future__ import annotations

import shutil
from pathlib import Path

from scripts.context_ci_support import exact, sources
from scripts.depot_removal_run import SELECTOR, RemovalRun
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import text
from scripts.grf_control_run import ControlRun
from scripts.order_state_evidence import command
from scripts.order_state_provenance import build as build_order_executables
from scripts.order_state_provenance import verify
from scripts.script_vm_provenance import select_executable
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


def snapshot(root: Path, output: Path, layout: Json) -> None:
    sources(root, layout)
    match at(layout, ("sources",)):
        case dict() as entries:
            for name in entries:
                path = output / "source" / name
                path.parent.mkdir(parents=True, exist_ok=True)
                _ = shutil.copy2(root / name, path)
        case _:
            raise WorldCheckError("Missing depot removal source pins")
    _ = shutil.copy2(root / "scripts/depot-removal-layout.json", output / "layout.json")


def build(control: ControlRun, target: Path) -> Json:
    result = build_order_executables(control, target)
    match result:
        case dict() as fields:
            fields["case_test"] = SELECTOR
            del fields["airport_test"]
        case _:
            raise WorldCheckError("Missing actual Cargo executable manifest")
    write_json(control.output / "binaries.json", result)
    return result


def verify_all(job: RemovalRun, layout: Json) -> None:
    verify(job.root, job.output, layout, job.binaries)
    cargo_bindings(job)
    match at(read_json(job.output / "provenance.json"), ("source_hashes",)):
        case dict() as entries if entries:
            for name, expected in entries.items():
                if digest(job.root / name) != expected:
                    raise WorldCheckError(
                        "Depot provenance source changed during execution"
                    )
        case _:
            raise WorldCheckError("Missing depot provenance source binding")


def cargo_bindings(job: RemovalRun) -> None:
    environment = read_json(job.output / "build-environment.json")
    if at(environment, ("CARGO_INCREMENTAL",)) != "0":
        raise WorldCheckError("Unexpected Cargo incremental environment")
    target = text(at(environment, ("CARGO_TARGET_DIR",)))
    for name, log, target_name, kind, test, argv in (
        (
            "runner",
            "cargo-test",
            "ottd_sim",
            "lib",
            True,
            [
                "cargo",
                "test",
                "--locked",
                "-p",
                "ottd-sim",
                "--lib",
                "--no-run",
                "--message-format=json",
            ],
        ),
        (
            "cli",
            "cargo-cli",
            "ottd",
            "bin",
            False,
            [
                "cargo",
                "build",
                "--locked",
                "-p",
                "ottd-cli",
                "--bin",
                "ottd",
                "--message-format=json",
            ],
        ),
    ):
        directory = job.output / "logs" / log
        command(directory, argv)
        if (
            at(read_json(directory / "environment.json"), ("CARGO_TARGET_DIR",))
            != target
        ):
            raise WorldCheckError("Actual Cargo target differs")
        selected = select_executable(
            (directory / "stdout.log").read_text(), target_name, kind, test=test
        )
        expected: Json = {
            "executable": str(job.output / "bin" / name),
            "original": str(selected),
            "sha256": digest(selected),
        }
        if not exact(at(job.binaries, (name,)), expected) or digest(
            Path(job.executable(name))
        ) != digest(selected):
            raise WorldCheckError(
                "Retained executable differs from actual Cargo selection"
            )
