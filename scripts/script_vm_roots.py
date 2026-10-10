# SPDX-License-Identifier: GPL-2.0-only
"""Configured native root sessions and production Runner admission."""

import shutil
import subprocess
import sys
from pathlib import Path
from typing import Final

from scripts.gameplay_foundations import FoundationRun
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

SOURCE: Final = Path("scripts/compat/script-vm/roots")
TESTS: Final = (
    "shared_root_and_independent_runner_state_survive_errors",
    "root_replacement_is_run_time_but_constants_are_compile_time",
    "supported_lookup_errors_do_not_conflate_native_unsupported_values",
    "host_preconditions_and_scalar_results_have_separate_ownership",
    "unsupported_root_source_remains_distinct_from_lexer_policy",
    "missing_out_of_scope_local_is_a_runtime_lookup",
    "checked_bytecode_keeps_root_identity_separate_from_scalar_operands",
)
BUILD: Final = [
    "cargo",
    "test",
    "--locked",
    "-p",
    "ottd-script",
    "--test",
    "root_runner",
    "--no-run",
    "--message-format=json",
]
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


def declarations() -> dict[str, dict[str, str]]:
    match read_json(ROOT / SOURCE / "witnesses.json"):
        case {"cases": list() as rows}:
            result: dict[str, dict[str, str]] = {}
            for row in rows:
                match row:
                    case {
                        "id": str() as name,
                        "session": str() as session,
                        "cwd": str() as cwd,
                        "capture": str() as capture,
                        "domain": str() as domain,
                    }:
                        if name in result:
                            raise WorldCheckError("Duplicate root session")
                        result[name] = {
                            "session": session,
                            "cwd": cwd,
                            "capture": capture,
                            "domain": domain,
                        }
                    case _:
                        raise WorldCheckError("Invalid root declaration")
        case _:
            raise WorldCheckError("Missing root declarations")
    for name, row in result.items():
        session = Path(row["session"])
        group = session.parts[0] if len(session.parts) == 3 else "."
        expected_id = (group + "-" if group != "." else "") + session.stem
        expected_capture = Path(group if group != "." else "native") / (
            session.stem + ".txt"
        )
        if (
            session.is_absolute()
            or ".." in session.parts
            or group not in {".", "ownership", "replacement"}
            or session.parent.name != "fixtures"
            or session.suffix != ".session"
            or row["cwd"] != group
            or name != expected_id
            or row["capture"] != str(expected_capture)
        ):
            raise WorldCheckError("Root session identity or relative path differs")
    sessions = {
        str(p.relative_to(ROOT / SOURCE)) for p in (ROOT / SOURCE).rglob("*.session")
    }
    if sessions != {row["session"] for row in result.values()}:
        raise WorldCheckError("Root declared session membership differs")
    domains = [row["domain"] for row in result.values()]
    if (
        len(result),
        domains.count("strict-stateful-projection"),
        domains.count("explicit-syntax-boundary"),
        domains.count("explicit-runtime-boundary"),
    ) != (90, 75, 9, 6):
        raise WorldCheckError("Incomplete root session domains")
    return result


def cases() -> tuple[str, ...]:
    return tuple(declarations())


def input_names() -> tuple[str, ...]:
    return tuple(
        sorted(
            str(p.relative_to(ROOT / SOURCE))
            for p in (ROOT / SOURCE).rglob("*")
            if p.is_file() and "fixtures" in p.relative_to(ROOT / SOURCE).parts
        )
    )


def argv(origin: Path, name: str) -> list[str]:
    row = declarations()[name]
    return [
        str(origin / "native/observe"),
        "--root-slots-session",
        str(Path(row["session"]).relative_to(row["cwd"])),
    ]


def capture(name: str) -> Path:
    return Path(declarations()[name]["capture"])


def working_directory(origin: Path, name: str) -> Path:
    return origin / "roots/inputs" / declarations()[name]["cwd"]


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


def validate_corpus() -> None:
    """Bind every native capture/input to the same production Rust session corpus."""
    match read_json(ROOT / SOURCE / "witnesses.json"):
        case {"files": dict() as files}:
            pass
        case _:
            raise WorldCheckError("Missing root corpus file map")
    rust = ROOT / "crates/ottd-script/tests/roots"
    actual = {
        str(p.relative_to(ROOT / SOURCE))
        for p in (ROOT / SOURCE).rglob("*")
        if p.is_file() and p.name != "witnesses.json"
    }
    if actual != set(files) or len(actual) != 212:
        raise WorldCheckError("Root corpus membership differs")
    for name, sha in files.items():
        if (
            not isinstance(sha, str)
            or digest(ROOT / SOURCE / name) != sha
            or digest(rust / name) != sha
        ):
            raise WorldCheckError("Root source or native capture differs")
    _ = declarations()


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


if __name__ == "__main__":
    sys.exit(main())
