# SPDX-License-Identifier: GPL-2.0-only
"""Native scalar-array sessions, explicit boundaries and production admission."""

import re
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

SOURCE: Final = Path("scripts/compat/script-vm/arrays")
TESTS: Final = (
    "scalar_storage_and_alias_identity",
    "all_host_insertions_reject_container_edges_before_publication",
    "root_affinity_is_checked_without_cloning_or_replacing_foreign_arrays",
    "returned_array_and_string_outlive_all_context_wrappers",
    "one_program_allocates_fresh_arrays_on_each_execution",
    "arrays_are_not_mutable_compiler_literals",
    "actual_native_scalar_idioms_and_defined_float_indices",
    "nested_arrays_are_reached_runtime_boundaries_not_lexer_policy",
    "unsupported_runtime_domains_and_actual_missing_indices_stay_distinct",
    "float_index_conversion_covers_defined_i64_edge_and_nonfinite_bits",
    "malformed_collection_bytecode_fails_without_unchecked_access",
)
BUILD: Final = [
    "cargo",
    "test",
    "--locked",
    "-p",
    "ottd-script",
    "--test",
    "arrays",
    "--no-run",
    "--message-format=json",
]
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
                            raise WorldCheckError("Duplicate array session")
                        result[name] = Session(
                            session, cwd, capture, domain, status, stderr
                        )
                    case _:
                        raise WorldCheckError("Invalid array declaration")
        case _:
            raise WorldCheckError("Missing array declarations")
    for name, row in result.items():
        session = Path(row.session)
        if (
            session.is_absolute()
            or ".." in session.parts
            or len(session.parts) != 3
            or row.cwd not in {"attempt-01", "attempt-02", "attempt-03"}
            or session.parent != Path(row.cwd) / "fixtures"
            or session.suffix != ".session"
            or name != row.cwd + "-" + session.stem
            or row.capture != str(Path(row.cwd) / "native" / (session.stem + ".txt"))
            or row.status != (65 if row.domain == "protocol-refusal" else 0)
            or bool(row.stderr) != (row.domain == "protocol-refusal")
        ):
            raise WorldCheckError("Array session identity or relative path differs")
    sessions = {
        str(p.relative_to(ROOT / SOURCE)) for p in (ROOT / SOURCE).rglob("*.session")
    }
    if sessions != {row.session for row in result.values()}:
        raise WorldCheckError("Array declared session membership differs")
    domains = [row.domain for row in result.values()]
    if (
        len(result),
        domains.count("strict-stateful-projection"),
        domains.count("explicit-runtime-boundary"),
        domains.count("protocol-refusal"),
    ) != (93, 78, 12, 3):
        raise WorldCheckError("Incomplete array session domains")
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
        "--array-session",
        str(Path(row.session).relative_to(row.cwd)),
    ]


def capture(name: str) -> Path:
    return Path(declarations()[name].capture)


def working_directory(origin: Path, name: str) -> Path:
    return origin / "arrays/inputs" / declarations()[name].cwd


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


def validate_session(directory: Path, origin: Path, name: str) -> None:
    output = directory / "arrays/sessions" / name
    if read_json(output / "argv.json") != argv(origin, name) or read_json(
        output / "cwd.json"
    ) != str(working_directory(origin, name)):
        raise WorldCheckError("Array session invocation differs")
    row = declarations()[name]
    if read_json(output / "process.json") != {
        "returncode": row.status,
        "expected": row.status,
    }:
        raise WorldCheckError("Array session process failed")
    if (output / "stderr.log").read_text() != row.stderr:
        raise WorldCheckError("Array session stderr differs")
    observed = (output / "stdout.log").read_text()
    expected = (ROOT / SOURCE / capture(name)).read_text()
    if comparable(name, observed) != comparable(name, expected):
        raise ArrayObservationMismatchError("Array session observation differs")


def comparable(name: str, text: str) -> str:
    """Exclude only the native pointer spelling in three declared boundary cases."""
    if name not in {
        "attempt-01-ordering-conversion-" + feed
        for feed in ("buffer", "unsigned", "utf8")
    }:
        return text
    lines = text.splitlines(keepends=True)
    addresses: list[bytes] = []
    for index, line in enumerate(lines):
        if not line.startswith(("return 9996 string ", "temp string ")):
            continue
        fields = line.split()
        size, encoded = fields[-2:]
        try:
            payload = bytes.fromhex(encoded)
            length = int(size)
        except ValueError as error:
            raise ArrayObservationMismatchError(
                "Invalid native pointer string"
            ) from error
        if (
            len(payload) != length
            or re.fullmatch(rb"\(array : 0x[0-9A-Fa-f]+\)", payload) is None
        ):
            raise ArrayObservationMismatchError("Invalid native pointer format/length")
        addresses.append(payload)
        lines[index] = " ".join(fields[:-2]) + " <native-array-address>\n"
    if len(addresses) != 2 or addresses[0] != addresses[1]:
        raise ArrayObservationMismatchError(
            "Native return/temp pointer identity differs"
        )
    return "".join(lines)


def validate_corpus() -> None:
    """Bind every native capture/input to the same production Rust session corpus."""
    match read_json(ROOT / SOURCE / "witnesses.json"):
        case {"files": dict() as files}:
            pass
        case _:
            raise WorldCheckError("Missing array corpus file map")
    rust = ROOT / "crates/ottd-script/tests/arrays-native"
    actual = {
        str(p.relative_to(ROOT / SOURCE))
        for p in (ROOT / SOURCE).rglob("*")
        if p.is_file() and p.name != "witnesses.json"
    }
    if actual != set(files) or len(actual) != 228:
        raise WorldCheckError("Array corpus membership differs")
    for name, sha in files.items():
        if (
            not isinstance(sha, str)
            or digest(ROOT / SOURCE / name) != sha
            or digest(rust / name) != sha
        ):
            raise WorldCheckError("Array source or native capture differs")
    _ = declarations()


class ArrayObservationMismatchError(WorldCheckError):
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


if __name__ == "__main__":
    sys.exit(main())
