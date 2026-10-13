# SPDX-License-Identifier: GPL-2.0-only
"""Roots public compatibility API and admission entrypoint."""

import sys
from pathlib import Path

from scripts.script_vm_roots_admission import (
    CONTROLS,
    SESSION_FILES,
    RootObservationMismatchError,
    admission_argv,
    admission_text,
    run_controls,
    validate_admission_receipts,
    validate_controls,
    validate_session,
)
from scripts.script_vm_roots_capture import observe, run_roots, validate_roots
from scripts.script_vm_roots_spec import (
    BUILD,
    SOURCE,
    TESTS,
    argv,
    capture,
    cases,
    declarations,
    input_names,
    validate_corpus,
    working_directory,
)
from scripts.world_check_support import ROOT, WorldCheckError


def main() -> int:
    match sys.argv[1:]:
        case ["--validate-session", directory, origin, name]:
            try:
                validate_session(Path(directory), Path(origin), name)
            except RootObservationMismatchError:
                print(admission_text(name, rejected=True), end="")
                return 1
            except WorldCheckError as error:
                print(f"ERROR root session validation: {error}", file=sys.stderr)
                return 2
            print(admission_text(name, rejected=False), end="")
            return 0
        case _:
            raise WorldCheckError("Expected --validate-session DIRECTORY ORIGIN NAME")


__all__ = (
    "BUILD",
    "CONTROLS",
    "ROOT",
    "SESSION_FILES",
    "SOURCE",
    "TESTS",
    "RootObservationMismatchError",
    "admission_argv",
    "admission_text",
    "argv",
    "capture",
    "cases",
    "declarations",
    "input_names",
    "main",
    "observe",
    "run_controls",
    "run_roots",
    "validate_admission_receipts",
    "validate_controls",
    "validate_corpus",
    "validate_roots",
    "validate_session",
    "working_directory",
)


if __name__ == "__main__":
    sys.exit(main())
