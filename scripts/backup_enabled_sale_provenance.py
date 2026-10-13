from __future__ import annotations

import shutil
from pathlib import Path

from scripts.backup_sale_run import BackupSaleRun
from scripts.context_ci_support import exact, sources
from scripts.gameplay_foundations import digest
from scripts.grf_control_run import ControlRun
from scripts.order_state_evidence import command
from scripts.order_state_provenance import verify
from scripts.script_vm_provenance import select_executable
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


def snapshot(root: Path, output: Path, layout: Json) -> None:
    sources(root, layout)
    match at(layout, ("sources",)):
        case dict() as entries:
            for name in entries:
                destination = output / "source" / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                _ = shutil.copy2(root / name, destination)
        case _:
            raise WorldCheckError("Missing enabled-sale source closure")
    _ = shutil.copy2(
        root / "scripts/backup-enabled-sale-layout.json", output / "layout.json"
    )


def build_argv(target: Path, *, cli: bool) -> list[str]:
    arguments = (
        ["build", "--locked", "-p", "ottd-cli", "--bin", "ottd"]
        if cli
        else ["test", "--locked", "-p", "ottd-sim", "--lib", "--no-run"]
    )
    return [
        "env",
        "CARGO_INCREMENTAL=0",
        "CARGO_PROFILE_DEV_DEBUG=0",
        "CARGO_PROFILE_TEST_DEBUG=0",
        f"CARGO_TARGET_DIR={target}",
        "cargo",
        *arguments,
        "--message-format=json",
    ]


def build(job: ControlRun, target: Path) -> Json:
    write_json(
        job.output / "build-environment.json",
        {
            "CARGO_INCREMENTAL": "0",
            "CARGO_PROFILE_DEV_DEBUG": "0",
            "CARGO_PROFILE_TEST_DEBUG": "0",
            "CARGO_TARGET_DIR": str(target),
        },
    )
    (job.output / "bin").mkdir()
    result: dict[str, Json] = {}
    for name, target_name, kind, is_test in (
        ("runner", "ottd_sim", "lib", True),
        ("cli", "ottd", "bin", False),
    ):
        built = job.run("cargo-" + name, build_argv(target, cli=not is_test))
        selected = select_executable(built.stdout, target_name, kind, test=is_test)
        destination = job.output / "bin" / name
        _ = shutil.copy2(selected, destination)
        destination.chmod(0o555)
        result[name] = {
            "executable": str(destination),
            "original": str(selected),
            "sha256": digest(destination),
        }
    write_json(job.output / "binaries.json", result)
    return result


def verify_all(job: BackupSaleRun, layout: Json) -> None:
    verify(job.root, job.output, layout, job.binaries)
    target_value = at(
        read_json(job.output / "build-environment.json"), ("CARGO_TARGET_DIR",)
    )
    if not isinstance(target_value, str):
        raise WorldCheckError("Missing Cargo target")
    expected: Json = {
        "CARGO_INCREMENTAL": "0",
        "CARGO_PROFILE_DEV_DEBUG": "0",
        "CARGO_PROFILE_TEST_DEBUG": "0",
        "CARGO_TARGET_DIR": target_value,
    }
    if not exact(read_json(job.output / "build-environment.json"), expected):
        raise WorldCheckError("Unexpected Cargo profile/environment")
    for name, target_name, kind, is_test in (
        ("runner", "ottd_sim", "lib", True),
        ("cli", "ottd", "bin", False),
    ):
        directory = job.output / "logs" / ("cargo-" + name)
        command(directory, build_argv(Path(target_value), cli=not is_test))
        selected = select_executable(
            (directory / "stdout.log").read_text(), target_name, kind, test=is_test
        )
        expected = {
            "executable": str(job.output / "bin" / name),
            "original": str(selected),
            "sha256": digest(selected),
        }
        if not exact(at(job.binaries, (name,)), expected) or digest(
            Path(job.executable(name))
        ) != digest(selected):
            raise WorldCheckError(
                "Enabled-sale executable differs from actual Cargo selection"
            )
    match at(read_json(job.output / "provenance.json"), ("source_hashes",)):
        case dict() as entries if entries:
            for name, sha in entries.items():
                if digest(job.root / name) != sha:
                    raise WorldCheckError("Native/source provenance changed")
        case _:
            raise WorldCheckError("Missing native/source provenance")
