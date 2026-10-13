# SPDX-License-Identifier: GPL-2.0-only
"""Arrays public compatibility API and admission entrypoint."""

import sys
from pathlib import Path

from scripts.script_vm_arrays_admission import (
    CONTROLS,
    SESSION_FILES,
    admission_argv,
    admission_text,
    run_controls,
    validate_admission_receipts,
    validate_controls,
)
from scripts.script_vm_arrays_capture import observe, run_arrays, validate_arrays
from scripts.script_vm_arrays_sessions import (
    ArrayObservationMismatchError,
    comparable,
    validate_session,
)
from scripts.script_vm_arrays_spec import (
    BUILD,
    SOURCE,
    TESTS,
    Session,
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
            except ArrayObservationMismatchError:
                print(admission_text(name, rejected=True), end="")
                return 1
            except WorldCheckError as error:
                print(f"ERROR array session validation: {error}", file=sys.stderr)
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
    "ArrayObservationMismatchError",
    "Session",
    "admission_argv",
    "admission_text",
    "argv",
    "capture",
    "cases",
    "comparable",
    "declarations",
    "input_names",
    "main",
    "observe",
    "run_arrays",
    "run_controls",
    "validate_admission_receipts",
    "validate_arrays",
    "validate_controls",
    "validate_corpus",
    "validate_session",
    "working_directory",
)


if __name__ == "__main__":
    sys.exit(main())
