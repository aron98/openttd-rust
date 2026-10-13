# SPDX-License-Identifier: GPL-2.0-only
"""Arrays admission controls and retained process validation."""

import shutil
from pathlib import Path
from typing import Final

from scripts.script_vm_arrays_sessions import (
    ArrayObservationMismatchError,
    validate_session,
)
from scripts.script_vm_provenance import digest
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    read_json,
    run,
    write_json,
)

CONTROLS: Final = {
    "construction": (
        "attempt-02-all-construction-boundaries-buffer",
        "element 1 0 string 1 78",
        "element 1 0 string 1 79",
    ),
    "set": (
        "attempt-02-set-before-after-buffer",
        "element 1 0 integer 2",
        "element 1 0 integer 99",
    ),
    "lifetime": (
        "attempt-01-child-temporary-buffer",
        "weak 1 null",
        "weak 1 array 1 length 1",
    ),
    "budget": (
        "attempt-02-all-construction-boundaries-buffer",
        "suspend -1 0",
        "suspend -1 1",
    ),
}


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
        "scripts.script_vm_arrays",
        "--validate-session",
        str(directory),
        str(origin),
        name,
    ]


def admission_text(name: str, *, rejected: bool) -> str:
    return (
        f"REJECT array session observation differs: {name}\n"
        if rejected
        else f"PASS array session admission: {name}\n"
    )


def run_controls(output: Path) -> None:
    for name, (session, old, new) in CONTROLS.items():
        control = output / "controls" / f"array-{name}"
        original = output / "arrays/sessions" / session
        accepted = run(
            admission_argv(output, output, session), control / "admit-original"
        )
        if (
            accepted.stdout != admission_text(session, rejected=False)
            or accepted.stderr
        ):
            raise WorldCheckError("Original array session was not admitted")
        mutant = control / "mutant"
        copied = mutant / "arrays/sessions" / session
        copied.mkdir(parents=True)
        before: dict[str, Json] = {}
        for filename in SESSION_FILES:
            before[filename] = digest(original / filename)
            _ = shutil.copy2(original / filename, copied / filename)
        source = (original / "stdout.log").read_text()
        if old not in source:
            raise WorldCheckError("Missing array mutation target")
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
            raise WorldCheckError("Array stdout mutation lacked its specific refusal")
        for filename in SESSION_FILES:
            if digest(original / filename) != before[filename]:
                raise WorldCheckError("Original array capture changed during control")
    validate_controls(output, output)


def validate_controls(directory: Path, origin: Path) -> None:
    for name, (session, old, new) in CONTROLS.items():
        control = directory / "controls" / f"array-{name}"
        original = directory / "arrays/sessions" / session
        mutant = control / "mutant"
        copied = mutant / "arrays/sessions" / session
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
            raise WorldCheckError("Array mutation source bindings differ")
        for filename in SESSION_FILES:
            expected = (original / filename).read_bytes()
            if filename == "stdout.log":
                if old not in expected.decode():
                    raise WorldCheckError("Array control target missing")
                expected = expected.decode().replace(old, new).encode()
            if (copied / filename).read_bytes() != expected:
                raise WorldCheckError(
                    "Array control changed more than its declared stdout"
                )
        try:
            validate_session(mutant, origin, session)
        except ArrayObservationMismatchError:
            pass
        else:
            raise WorldCheckError("Array stdout mutation was admitted")
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
            raise WorldCheckError("Array control admission receipt differs")
