# SPDX-License-Identifier: GPL-2.0-only
"""Arrays capture production and executable binding."""

import shutil
import subprocess
from pathlib import Path

from scripts.gameplay_foundations import FoundationRun
from scripts.script_vm_arrays_admission import run_controls, validate_controls
from scripts.script_vm_arrays_sessions import validate_session
from scripts.script_vm_arrays_spec import (
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
from scripts.script_vm_observation import require_tests
from scripts.script_vm_provenance import digest, select_executable
from scripts.world_check_support import (
    ROOT,
    Json,
    WorldCheckError,
    read_json,
    run,
    write_json,
)


def observe(origin: Path, name: str) -> None:
    """Retain the real session working directory and every native process stream."""
    destination = origin / "arrays/sessions" / name
    destination.mkdir(parents=True)
    arguments = argv(origin, name)
    cwd = working_directory(origin, name)
    write_json(destination / "argv.json", list(arguments))
    write_json(destination / "cwd.json", str(cwd))
    result = subprocess.run(
        arguments, cwd=cwd, capture_output=True, text=True, timeout=65, check=False
    )
    _ = (destination / "stdout.log").write_text(result.stdout)
    _ = (destination / "stderr.log").write_text(result.stderr)
    write_json(
        destination / "process.json",
        {"returncode": result.returncode, "expected": declarations()[name].status},
    )
    validate_session(origin, origin, name)


def run_arrays(builder: FoundationRun) -> None:
    validate_corpus()
    output = builder.output
    built = builder.run("build-array-tests", BUILD)
    tests = select_executable(built.stdout, "arrays", "test", test=True)
    target = output / "bin/array-tests"
    _ = shutil.copy2(tests, target)
    target.chmod(0o555)
    identity: dict[str, Json] = {"path": str(tests), "sha256": digest(tests)}
    write_json(output / "array-identities.json", identity)
    observed = run([str(tests), "--test-threads=1"], output / "array-tests")
    require_tests(observed.stdout, TESTS)
    destination = output / "arrays/inputs"
    destination.mkdir(parents=True)
    for name in input_names():
        target = destination / name
        target.parent.mkdir(parents=True, exist_ok=True)
        _ = shutil.copy2(ROOT / SOURCE / name, target)
    for name in cases():
        observe(output, name)
        target = destination / capture(name)
        target.parent.mkdir(parents=True, exist_ok=True)
        _ = shutil.copy2(output / "arrays/sessions" / name / "stdout.log", target)
    run_controls(output)
    if (
        digest(tests) != identity["sha256"]
        or digest(output / "bin/array-tests") != identity["sha256"]
    ):
        raise WorldCheckError("Array test executable changed")
    write_json(output / "array-identity-after.json", identity)


def validate_arrays(directory: Path, origin: Path) -> None:
    validate_corpus()
    match read_json(directory / "array-identities.json"):
        case {"path": str() as tests, "sha256": str() as sha}:
            pass
        case _:
            raise WorldCheckError("Missing array test identity")
    if (
        read_json(directory / "array-identity-after.json")
        != {"path": tests, "sha256": sha}
        or digest(directory / "bin/array-tests") != sha
    ):
        raise WorldCheckError("Array test executable changed")
    if (
        read_json(directory / "logs/build-array-tests/argv.json") != BUILD
        or str(
            select_executable(
                (directory / "logs/build-array-tests/stdout.log").read_text(),
                "arrays",
                "test",
                test=True,
            )
        )
        != tests
    ):
        raise WorldCheckError("Array test build selection differs")
    if read_json(directory / "array-tests/argv.json") != [tests, "--test-threads=1"]:
        raise WorldCheckError("Array test invocation differs")
    require_tests((directory / "array-tests/stdout.log").read_text(), TESTS)
    for name in input_names():
        if digest(directory / "arrays/inputs" / name) != digest(ROOT / SOURCE / name):
            raise WorldCheckError("Array session input changed")
    for name in cases():
        validate_session(directory, origin, name)
        if (directory / "arrays/inputs" / capture(name)).read_bytes() != (
            directory / "arrays/sessions" / name / "stdout.log"
        ).read_bytes():
            raise WorldCheckError(
                "Rust array capture differs from actual native process"
            )
    validate_controls(directory, origin)
