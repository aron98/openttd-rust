# SPDX-License-Identifier: GPL-2.0-only
"""Roots capture production and executable binding."""

import shutil
import subprocess
from pathlib import Path

from scripts.gameplay_foundations import FoundationRun
from scripts.script_vm_observation import require_tests
from scripts.script_vm_provenance import digest, select_executable
from scripts.script_vm_roots_admission import (
    run_controls,
    validate_controls,
    validate_session,
)
from scripts.script_vm_roots_spec import (
    BUILD,
    SOURCE,
    TESTS,
    argv,
    capture,
    cases,
    input_names,
    validate_corpus,
    working_directory,
)
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
    destination = origin / "roots/sessions" / name
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
        destination / "process.json", {"returncode": result.returncode, "expected": 0}
    )
    if (
        result.returncode
        or result.stderr
        or result.stdout != (ROOT / SOURCE / capture(name)).read_text()
    ):
        raise WorldCheckError(f"Native root session differs: {name}")


def run_roots(builder: FoundationRun) -> None:
    validate_corpus()
    output = builder.output
    built = builder.run("build-root-tests", BUILD)
    tests = select_executable(built.stdout, "root_runner", "test", test=True)
    target = output / "bin/root-tests"
    _ = shutil.copy2(tests, target)
    target.chmod(0o555)
    identity: dict[str, Json] = {"path": str(tests), "sha256": digest(tests)}
    write_json(output / "root-identities.json", identity)
    observed = run([str(tests), "--test-threads=1"], output / "root-tests")
    require_tests(observed.stdout, TESTS)
    destination = output / "roots/inputs"
    destination.mkdir(parents=True)
    for name in input_names():
        target = destination / name
        target.parent.mkdir(parents=True, exist_ok=True)
        _ = shutil.copy2(ROOT / SOURCE / name, target)
    for name in cases():
        observe(output, name)
        target = destination / capture(name)
        target.parent.mkdir(parents=True, exist_ok=True)
        _ = shutil.copy2(output / "roots/sessions" / name / "stdout.log", target)
    run_controls(output)
    if (
        digest(tests) != identity["sha256"]
        or digest(output / "bin/root-tests") != identity["sha256"]
    ):
        raise WorldCheckError("Root test executable changed")
    write_json(output / "root-identity-after.json", identity)


def validate_roots(directory: Path, origin: Path) -> None:
    validate_corpus()
    match read_json(directory / "root-identities.json"):
        case {"path": str() as tests, "sha256": str() as sha}:
            pass
        case _:
            raise WorldCheckError("Missing root test identity")
    if (
        read_json(directory / "root-identity-after.json")
        != {"path": tests, "sha256": sha}
        or digest(directory / "bin/root-tests") != sha
    ):
        raise WorldCheckError("Root test executable changed")
    if (
        read_json(directory / "logs/build-root-tests/argv.json") != BUILD
        or str(
            select_executable(
                (directory / "logs/build-root-tests/stdout.log").read_text(),
                "root_runner",
                "test",
                test=True,
            )
        )
        != tests
    ):
        raise WorldCheckError("Root test build selection differs")
    if read_json(directory / "root-tests/argv.json") != [tests, "--test-threads=1"]:
        raise WorldCheckError("Root test invocation differs")
    require_tests((directory / "root-tests/stdout.log").read_text(), TESTS)
    for name in input_names():
        if digest(directory / "roots/inputs" / name) != digest(ROOT / SOURCE / name):
            raise WorldCheckError("Root session input changed")
    for name in cases():
        validate_session(directory, origin, name)
        if (directory / "roots/inputs" / capture(name)).read_bytes() != (
            directory / "roots/sessions" / name / "stdout.log"
        ).read_bytes():
            raise WorldCheckError(
                "Rust root capture differs from actual native process"
            )
    validate_controls(directory, origin)
