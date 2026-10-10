# SPDX-License-Identifier: GPL-2.0-only
"""Native array foreach sessions, explicit boundaries and production admission."""

import shutil
import subprocess
import sys
from dataclasses import dataclass
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

SOURCE: Final = Path("scripts/compat/script-vm/foreach")
TESTS: Final = (
    "valid_cursor_success_and_exhaustion",
    "malformed_registers_reserved_operand_and_overlap_fail",
    "only_taken_branch_is_bounds_checked",
    "cursor_type_is_bytecode_error_not_a_new_native_policy",
    "unsupported_iterable_and_non_iterable_have_distinct_boundaries",
    "successful_skip_is_checked_and_receiver_precedes_cursor_type",
    "production_regressions::allocation_iteration_and_index_mutation",
    "production_regressions::shared_root_mutation_at_foreach_boundary",
)

BUILD: Final = [
    "cargo",
    "test",
    "--locked",
    "-p",
    "ottd-script",
    "--test",
    "foreach_bytecode",
    "--no-run",
    "--message-format=json",
]
CONTROLS: Final = {
    "cursor": (
        "attempt-02-future-host-write-0-buffer",
        "frame 5 integer 1",
        "frame 5 integer 99",
    ),
    "skipped-post": (
        "attempt-02-future-host-write-0-buffer",
        "suspend 0 5",
        "suspend 0 4",
    ),
    "temporary": (
        "attempt-02-future-host-write-0-buffer",
        "temp array 1 length 3",
        "temp null",
    ),
    "lifetime": (
        "attempt-02-child-owner-0-buffer",
        "weak 1 null",
        "weak 1 array 1 length 2",
    ),
    "root-rebinding": (
        "attempt-03-root-replacement-0-buffer",
        "return 9987 integer 6",
        "return 9987 integer 13",
    ),
    "current-value": (
        "attempt-03-current-host-write-0-buffer",
        "return 9991 string 7 6f6c646e657874",
        "return 9991 string 7 6e65776e657874",
    ),
}


@dataclass(frozen=True, slots=True)
class Session:
    session: str
    cwd: str
    capture: str
    domain: str
    status: int
    stderr: str


def declarations() -> dict[str, Session]:
    match read_json(ROOT / SOURCE / "witnesses.json"):
        case {"cases": list() as rows}:
            result: dict[str, Session] = {}
            for row in rows:
                match row:
                    case {
                        "id": str() as name,
                        "session": str() as session,
                        "cwd": str() as cwd,
                        "capture": str() as capture,
                        "domain": str() as domain,
                        "status": int() as status,
                        "stderr": str() as stderr,
                    }:
                        if name in result:
                            raise WorldCheckError("Duplicate foreach session")
                        result[name] = Session(
                            session, cwd, capture, domain, status, stderr
                        )
                    case _:
                        raise WorldCheckError("Invalid foreach declaration")
        case _:
            raise WorldCheckError("Missing foreach declarations")
    for name, row in result.items():
        session = Path(row.session)
        if (
            session.is_absolute()
            or ".." in session.parts
            or len(session.parts) != 3
            or row.cwd not in {"attempt-01", "attempt-02", "attempt-03", "attempt-04"}
            or session.parent != Path(row.cwd) / "fixtures"
            or session.suffix != ".session"
            or name != row.cwd + "-" + session.stem
            or row.capture != expected_capture(row, session)
            or row.status != 0
            or row.stderr != ""
        ):
            raise WorldCheckError("Foreach session identity or relative path differs")
    sessions = {
        str(p.relative_to(ROOT / SOURCE)) for p in (ROOT / SOURCE).rglob("*.session")
    }
    if sessions != {row.session for row in result.values()}:
        raise WorldCheckError("Foreach declared session membership differs")
    domains = [row.domain for row in result.values()]
    if (
        len(result),
        domains.count("strict-stateful-projection"),
        domains.count("explicit-runtime-boundary"),
    ) != (135, 129, 6):
        raise WorldCheckError("Incomplete foreach session domains")
    return result


def cases() -> tuple[str, ...]:
    return tuple(declarations())


def input_names() -> tuple[str, ...]:
    return tuple(
        sorted(
            str(p.relative_to(ROOT / SOURCE))
            for p in (ROOT / SOURCE).rglob("*")
            if p.is_file() and p.suffix in {".nut", ".session"}
        )
    )


def argv(origin: Path, name: str) -> list[str]:
    row = declarations()[name]
    return [
        str(origin / "native/observe"),
        "--array-session",
        str(Path(row.session).relative_to(row.cwd)),
    ]


def capture(name: str) -> Path:
    return Path(declarations()[name].capture)


def working_directory(origin: Path, name: str) -> Path:
    return origin / "foreach/inputs" / declarations()[name].cwd


def observe(origin: Path, name: str) -> None:
    """Retain the real session working directory and every native process stream."""
    destination = origin / "foreach/sessions" / name
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


def run_foreach(builder: FoundationRun) -> None:
    validate_corpus()
    output = builder.output
    built = builder.run("build-foreach-tests", BUILD)
    tests = select_executable(built.stdout, "foreach_bytecode", "test", test=True)
    target = output / "bin/foreach-tests"
    _ = shutil.copy2(tests, target)
    target.chmod(0o555)
    identity: dict[str, Json] = {"path": str(tests), "sha256": digest(tests)}
    write_json(output / "foreach-identities.json", identity)
    observed = run([str(tests), "--test-threads=1"], output / "foreach-tests")
    require_tests(observed.stdout, TESTS)
    destination = output / "foreach/inputs"
    destination.mkdir(parents=True)
    for name in input_names():
        target = destination / name
        target.parent.mkdir(parents=True, exist_ok=True)
        _ = shutil.copy2(ROOT / SOURCE / name, target)
    for name in cases():
        observe(output, name)
        target = destination / capture(name)
        target.parent.mkdir(parents=True, exist_ok=True)
        _ = shutil.copy2(output / "foreach/sessions" / name / "stdout.log", target)
    run_controls(output)
    if (
        digest(tests) != identity["sha256"]
        or digest(output / "bin/foreach-tests") != identity["sha256"]
    ):
        raise WorldCheckError("Foreach test executable changed")
    write_json(output / "foreach-identity-after.json", identity)


def validate_foreach(directory: Path, origin: Path) -> None:
    validate_corpus()
    match read_json(directory / "foreach-identities.json"):
        case {"path": str() as tests, "sha256": str() as sha}:
            pass
        case _:
            raise WorldCheckError("Missing foreach test identity")
    if (
        read_json(directory / "foreach-identity-after.json")
        != {"path": tests, "sha256": sha}
        or digest(directory / "bin/foreach-tests") != sha
    ):
        raise WorldCheckError("Foreach test executable changed")
    if (
        read_json(directory / "logs/build-foreach-tests/argv.json") != BUILD
        or str(
            select_executable(
                (directory / "logs/build-foreach-tests/stdout.log").read_text(),
                "foreach_bytecode",
                "test",
                test=True,
            )
        )
        != tests
    ):
        raise WorldCheckError("Foreach test build selection differs")
    if read_json(directory / "foreach-tests/argv.json") != [tests, "--test-threads=1"]:
        raise WorldCheckError("Foreach test invocation differs")
    require_tests((directory / "foreach-tests/stdout.log").read_text(), TESTS)
    for name in input_names():
        if digest(directory / "foreach/inputs" / name) != digest(ROOT / SOURCE / name):
            raise WorldCheckError("Foreach session input changed")
    for name in cases():
        validate_session(directory, origin, name)
        if (directory / "foreach/inputs" / capture(name)).read_bytes() != (
            directory / "foreach/sessions" / name / "stdout.log"
        ).read_bytes():
            raise WorldCheckError(
                "Rust foreach capture differs from actual native process"
            )
    validate_controls(directory, origin)


def validate_session(directory: Path, origin: Path, name: str) -> None:
    output = directory / "foreach/sessions" / name
    if read_json(output / "argv.json") != argv(origin, name) or read_json(
        output / "cwd.json"
    ) != str(working_directory(origin, name)):
        raise WorldCheckError("Foreach session invocation differs")
    row = declarations()[name]
    if read_json(output / "process.json") != {
        "returncode": row.status,
        "expected": row.status,
    }:
        raise WorldCheckError("Foreach session process failed")
    if (output / "stderr.log").read_text() != row.stderr:
        raise WorldCheckError("Foreach session stderr differs")
    observed = (output / "stdout.log").read_text()
    expected = (ROOT / SOURCE / capture(name)).read_text()
    if comparable(name, observed) != comparable(name, expected):
        raise ForeachObservationMismatchError("Foreach session observation differs")


def comparable(name: str, text: str) -> str:
    if name not in declarations():
        raise WorldCheckError("Unknown foreach session")
    return text


def expected_capture(row: Session, session: Path) -> str:
    if row.domain == "explicit-runtime-boundary":
        if row.cwd not in {"attempt-01", "attempt-02"} or session.stem not in {
            "string-iterable-0-" + feed for feed in ("buffer", "unsigned", "utf8")
        }:
            raise WorldCheckError("Unknown foreach runtime boundary")
        feed = session.stem.rsplit("-", 1)[1]
        return f"boundaries/{row.cwd}-{feed}.txt"
    if row.domain != "strict-stateful-projection":
        raise WorldCheckError("Unknown foreach domain")
    return str(Path(row.cwd) / "native" / (session.stem + ".txt"))


def validate_corpus() -> None:
    """Bind every native capture/input to the same production Rust session corpus."""
    match read_json(ROOT / SOURCE / "witnesses.json"):
        case {"files": dict() as files}:
            pass
        case _:
            raise WorldCheckError("Missing foreach corpus file map")
    rust = ROOT / "crates/ottd-script/tests/foreach-native"
    actual = {
        str(p.relative_to(ROOT / SOURCE))
        for p in (ROOT / SOURCE).rglob("*")
        if p.is_file() and p.name != "witnesses.json"
    }
    if actual != set(files) or len(actual) != 317:
        raise WorldCheckError("Foreach corpus membership differs")
    for name, sha in files.items():
        if (
            not isinstance(sha, str)
            or digest(ROOT / SOURCE / name) != sha
            or digest(rust / name) != sha
        ):
            raise WorldCheckError("Foreach source or native capture differs")
    _ = declarations()


class ForeachObservationMismatchError(WorldCheckError):
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
        "scripts.script_vm_foreach",
        "--validate-session",
        str(directory),
        str(origin),
        name,
    ]


def admission_text(name: str, *, rejected: bool) -> str:
    return (
        f"REJECT foreach session observation differs: {name}\n"
        if rejected
        else f"PASS foreach session admission: {name}\n"
    )


def run_controls(output: Path) -> None:
    for name, (session, old, new) in CONTROLS.items():
        control = output / "controls" / f"foreach-{name}"
        original = output / "foreach/sessions" / session
        accepted = run(
            admission_argv(output, output, session), control / "admit-original"
        )
        if (
            accepted.stdout != admission_text(session, rejected=False)
            or accepted.stderr
        ):
            raise WorldCheckError("Original foreach session was not admitted")
        mutant = control / "mutant"
        copied = mutant / "foreach/sessions" / session
        copied.mkdir(parents=True)
        before: dict[str, Json] = {}
        for filename in SESSION_FILES:
            before[filename] = digest(original / filename)
            _ = shutil.copy2(original / filename, copied / filename)
        source = (original / "stdout.log").read_text()
        if old not in source:
            raise WorldCheckError("Missing foreach mutation target")
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
            raise WorldCheckError("Foreach stdout mutation lacked its specific refusal")
        for filename in SESSION_FILES:
            if digest(original / filename) != before[filename]:
                raise WorldCheckError("Original foreach capture changed during control")
    validate_controls(output, output)


def validate_controls(directory: Path, origin: Path) -> None:
    for name, (session, old, new) in CONTROLS.items():
        control = directory / "controls" / f"foreach-{name}"
        original = directory / "foreach/sessions" / session
        mutant = control / "mutant"
        copied = mutant / "foreach/sessions" / session
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
            raise WorldCheckError("Foreach mutation source bindings differ")
        for filename in SESSION_FILES:
            expected = (original / filename).read_bytes()
            if filename == "stdout.log":
                if old not in expected.decode():
                    raise WorldCheckError("Foreach control target missing")
                expected = expected.decode().replace(old, new).encode()
            if (copied / filename).read_bytes() != expected:
                raise WorldCheckError(
                    "Foreach control changed more than its declared stdout"
                )
        try:
            validate_session(mutant, origin, session)
        except ForeachObservationMismatchError:
            pass
        else:
            raise WorldCheckError("Foreach stdout mutation was admitted")
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
            raise WorldCheckError("Foreach control admission receipt differs")


def main() -> int:
    match sys.argv[1:]:
        case ["--validate-session", directory, origin, name]:
            try:
                validate_session(Path(directory), Path(origin), name)
            except ForeachObservationMismatchError:
                print(admission_text(name, rejected=True), end="")
                return 1
            except WorldCheckError as error:
                print(f"ERROR foreach session validation: {error}", file=sys.stderr)
                return 2
            print(admission_text(name, rejected=False), end="")
            return 0
        case _:
            raise WorldCheckError("Expected --validate-session DIRECTORY ORIGIN NAME")


if __name__ == "__main__":
    sys.exit(main())
