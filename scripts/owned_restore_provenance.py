from __future__ import annotations

import shutil
from pathlib import Path

from scripts.backup_enabled_sale_provenance import build_argv
from scripts.context_ci_support import exact, sources
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import text
from scripts.order_state_evidence import command
from scripts.order_state_provenance import verify
from scripts.owned_restore_run import RestoreRun
from scripts.owned_restore_sources import verify_compiled
from scripts.script_vm_provenance import select_executable
from scripts.world_check_support import Json, WorldCheckError, at, read_json


def snapshot(root: Path, output: Path, layout: Json) -> None:
    sources(root, layout)
    verify_compiled(root, at(layout, ("sources",)))
    match at(layout, ("sources",)):
        case dict() as entries:
            for name in entries:
                destination = output / "source" / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                _ = shutil.copy2(root / name, destination)
        case _:
            raise WorldCheckError("Missing Restore source closure")
    _ = shutil.copy2(root / "scripts/owned-restore-layout.json", output / "layout.json")


def verify_all(job: RestoreRun, layout: Json) -> None:
    verify_compiled(job.root, at(layout, ("sources",)))
    verify(job.root, job.output, layout, job.binaries)
    build_environment = read_json(job.output / "build-environment.json")
    target = Path(text(at(build_environment, ("CARGO_TARGET_DIR",))))
    expected: Json = {
        "CARGO_INCREMENTAL": "0",
        "CARGO_PROFILE_DEV_DEBUG": "0",
        "CARGO_PROFILE_TEST_DEBUG": "0",
        "CARGO_TARGET_DIR": str(target),
    }
    if not exact(expected, build_environment):
        raise WorldCheckError("Restore Cargo profile/environment differs")
    for name, target_name, kind, is_test in (
        ("runner", "ottd_sim", "lib", True),
        ("cli", "ottd", "bin", False),
    ):
        directory = job.output / "logs" / ("cargo-" + name)
        command(directory, build_argv(target, cli=not is_test))
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
                "Restore executable differs from actual Cargo selection"
            )
    match at(read_json(job.output / "provenance.json"), ("source_hashes",)):
        case dict() as entries if entries:
            for name, sha in entries.items():
                if digest(job.root / name) != sha:
                    raise WorldCheckError("Restore source changed during execution")
        case _:
            raise WorldCheckError("Missing Restore source-before identity")
