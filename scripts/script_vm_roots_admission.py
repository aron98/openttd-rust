# SPDX-License-Identifier: GPL-2.0-only
"""Roots admission controls and retained process validation."""

import shutil
from pathlib import Path
from typing import Final

from scripts.script_vm_provenance import digest
from scripts.script_vm_roots_spec import SOURCE, argv, capture, working_directory
from scripts.world_check_support import (
    ROOT,
    Json,
    WorldCheckError,
    read_json,
    run,
    write_json,
)

CONTROLS: Final = {
    "write": (
        "failure-persistence-buffer",
        "raw 73636f7265 integer 7",
        "raw 73636f7265 integer 1",
    ),
    "temporary": (
        "ownership-compile-temp-buffer",
        "temp unsupported 134217984",
        "temp null",
    ),
    "identity": (
        "replacement-buffer",
        "raw 73636f7265 integer 2",
        "raw 73636f7265 integer 1",
    ),
    "budget": ("budget-every-step-buffer", "suspend -1 0", "suspend -1 1"),
}


def validate_session(directory: Path, origin: Path, name: str) -> None:
    output = directory / "roots/sessions" / name
    if read_json(output / "argv.json") != argv(origin, name) or read_json(
        output / "cwd.json"
    ) != str(working_directory(origin, name)):
        raise WorldCheckError("Root session invocation differs")
    if read_json(output / "process.json") != {"returncode": 0, "expected": 0}:
        raise WorldCheckError("Root session process failed")
    if (output / "stderr.log").read_text():
        raise WorldCheckError("Root session stderr differs")
    if (output / "stdout.log").read_text() != (
        ROOT / SOURCE / capture(name)
    ).read_text():
        raise RootObservationMismatchError("Root session observation differs")


class RootObservationMismatchError(WorldCheckError):
    """The original session identity passed but its stdout differs from native."""


SESSION_FILES: Final = (
    "argv.json",
    "cwd.json",
    "process.json",
    "stdout.log",
    "stderr.log",
)


def admission_argv(directory: Path, origin: Path, name: str) -> list[str]:
    return [
        "python3",
        "-m",
        "scripts.script_vm_roots",
        "--validate-session",
        str(directory),
        str(origin),
        name,
    ]


def admission_text(name: str, *, rejected: bool) -> str:
    return (
        f"REJECT root session observation differs: {name}\n"
        if rejected
        else f"PASS root session admission: {name}\n"
    )


def run_controls(output: Path) -> None:
    for name, (session, old, new) in CONTROLS.items():
        control = output / "controls" / f"root-{name}"
        original = output / "roots/sessions" / session
        accepted = run(
            admission_argv(output, output, session), control / "admit-original"
        )
        if (
            accepted.stdout != admission_text(session, rejected=False)
            or accepted.stderr
        ):
            raise WorldCheckError("Original root session was not admitted")
        mutant = control / "mutant"
        copied = mutant / "roots/sessions" / session
        copied.mkdir(parents=True)
        before: dict[str, Json] = {}
        for filename in SESSION_FILES:
            before[filename] = digest(original / filename)
            _ = shutil.copy2(original / filename, copied / filename)
        source = (original / "stdout.log").read_text()
        if old not in source:
            raise WorldCheckError("Missing root mutation target")
        _ = (copied / "stdout.log").write_text(source.replace(old, new))
        write_json(
            control / "mutation.json",
            {
                "session": session,
                "old": old,
                "new": new,
                "before": before,
                "after_stdout": digest(copied / "stdout.log"),
            },
        )
        refused = run(
            admission_argv(mutant, output, session), control / "admit-mutant", 1
        )
        if refused.stdout != admission_text(session, rejected=True) or refused.stderr:
            raise WorldCheckError("Root stdout mutation lacked its specific refusal")
        for filename in SESSION_FILES:
            if digest(original / filename) != before[filename]:
                raise WorldCheckError("Original root capture changed during control")
    validate_controls(output, output)


def validate_controls(directory: Path, origin: Path) -> None:
    for name, (session, old, new) in CONTROLS.items():
        control = directory / "controls" / f"root-{name}"
        original = directory / "roots/sessions" / session
        mutant = control / "mutant"
        copied = mutant / "roots/sessions" / session
        validate_session(directory, origin, session)
        before: dict[str, Json] = {
            filename: digest(original / filename) for filename in SESSION_FILES
        }
        if read_json(control / "mutation.json") != {
            "session": session,
            "old": old,
            "new": new,
            "before": before,
            "after_stdout": digest(copied / "stdout.log"),
        }:
            raise WorldCheckError("Root mutation source bindings differ")
        for filename in SESSION_FILES:
            expected = (original / filename).read_bytes()
            if filename == "stdout.log":
                if old not in expected.decode():
                    raise WorldCheckError("Root control target missing")
                expected = expected.decode().replace(old, new).encode()
            if (copied / filename).read_bytes() != expected:
                raise WorldCheckError(
                    "Root control changed more than its declared stdout"
                )
        try:
            validate_session(mutant, origin, session)
        except RootObservationMismatchError:
            pass
        else:
            raise WorldCheckError("Root stdout mutation was admitted")
        validate_admission_receipts(control, directory, origin, session)


def validate_admission_receipts(
    control: Path, directory: Path, origin: Path, session: str
) -> None:
    mutant = control / "mutant"
    for label, selected, status in (
        ("admit-original", origin, 0),
        ("admit-mutant", origin / mutant.relative_to(directory), 1),
    ):
        log = control / label
        if (
            read_json(log / "argv.json") != admission_argv(selected, origin, session)
            or read_json(log / "process.json")
            != {"returncode": status, "expected": status}
            or (log / "stdout.log").read_text()
            != admission_text(session, rejected=bool(status))
            or (log / "stderr.log").read_text()
        ):
            raise WorldCheckError("Root control admission receipt differs")
