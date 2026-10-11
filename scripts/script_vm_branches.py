# SPDX-License-Identifier: GPL-2.0-only
"""Branches public compatibility API and admission entrypoint."""

from pathlib import Path

from scripts.script_vm_branches_capture import finish_branches, run_branches
from scripts.script_vm_branches_spec import (
    BRANCH_CONTROLS,
    BUDGETS,
    FRAME_CREDITS,
    FRAME_SCHEDULES,
    FRAME_TESTS,
    FRAMES,
    LIB_BUILD,
    fixture_name,
)
from scripts.script_vm_observation import compare_observation, require_tests
from scripts.script_vm_provenance import digest, select_executable
from scripts.world_check_support import ROOT, WorldCheckError, read_json


def validate_branches(directory: Path) -> None:
    identities = read_json(directory / "identities.json")
    match identities:
        case {"artifact_root": str() as root, "rust": {"path": str() as rust}}:
            origin = Path(root)
        case _:
            raise WorldCheckError("Missing branch observer identities")
    match read_json(directory / "branch-identities.json"):
        case {"path": str() as tests, "sha256": str() as sha}:
            pass
        case _:
            raise WorldCheckError("Missing private frame test identity")
    if (
        read_json(directory / "branch-identity-after.json")
        != {"path": tests, "sha256": sha}
        or digest(directory / "bin/lib-tests") != sha
    ):
        raise WorldCheckError("Private frame test executable changed")
    if (
        read_json(directory / "logs/build-libtests/argv.json") != LIB_BUILD
        or str(
            select_executable(
                (directory / "logs/build-libtests/stdout.log").read_text(),
                "ottd_script",
                "lib",
                test=True,
            )
        )
        != tests
    ):
        raise WorldCheckError("Private frame test build selection differs")
    if read_json(directory / "lib-tests/argv.json") != [
        "env",
        f"OTTD_SCRIPT_ROOT_CORPUS={origin / 'roots/inputs'}",
        f"OTTD_SCRIPT_ARRAY_CORPUS={origin / 'arrays/inputs'}",
        f"OTTD_SCRIPT_FOREACH_CORPUS={origin / 'foreach/inputs'}",
        tests,
        "--test-threads=1",
        "--exact",
        *FRAME_TESTS,
    ]:
        raise WorldCheckError("Private frame test selection differs")
    require_tests((directory / "lib-tests/stdout.log").read_text(), FRAME_TESTS)
    native = str(origin / "native/observe")
    _validate_frames(directory, origin, native)
    for index, (name, budget, stage) in enumerate(BUDGETS):
        destination = directory / "budgets" / str(index)
        for side, binary in (("native", native), ("rust", rust)):
            if read_json(destination / side / "argv.json") != [
                binary,
                str(origin / "inputs" / fixture_name(name)),
                *map(str, budget),
            ]:
                raise WorldCheckError("Branch budget selection differs")
        compare_observation(
            (destination / "native/stdout.log").read_text(),
            (destination / "rust/stdout.log").read_text(),
            stage,
        )
    _validate_corruptions(directory, origin)


def _validate_frames(directory: Path, origin: Path, native: str) -> None:
    for name in FRAMES:
        destination = directory / "frames" / name
        argv = [
            native,
            str(origin / "inputs" / fixture_name(name)),
            "--frames",
            *map(str, FRAME_SCHEDULES.get(name, FRAME_CREDITS)),
        ]
        if (
            read_json(destination / "argv.json") != argv
            or (destination / "stdout.log").read_text()
            != (ROOT / f"crates/ottd-script/tests/frames/{name}.txt").read_text()
        ):
            raise WorldCheckError("Native private frame witness differs")


def _validate_corruptions(directory: Path, origin: Path) -> None:
    for name, (fixture, old, new) in BRANCH_CONTROLS.items():
        relative = f"cases/{Path(fixture_name(fixture)).stem}/10000/native/stdout.log"
        base = directory / relative
        changed = directory / "controls" / f"branch-{name}" / "changed.stdout"
        expected_argv = [
            "diff",
            "-u",
            str(origin / relative),
            str(origin / changed.relative_to(directory)),
        ]
        if (
            old not in base.read_text()
            or changed.read_text() != base.read_text().replace(old, new)
            or read_json(changed.parent / "compare/argv.json") != expected_argv
            or not (changed.parent / "compare/stdout.log").read_text()
        ):
            raise WorldCheckError("Branch corruption was not compared")


__all__ = (
    "BRANCH_CONTROLS",
    "BUDGETS",
    "FRAMES",
    "FRAME_CREDITS",
    "FRAME_SCHEDULES",
    "FRAME_TESTS",
    "LIB_BUILD",
    "finish_branches",
    "fixture_name",
    "run_branches",
    "validate_branches",
)
