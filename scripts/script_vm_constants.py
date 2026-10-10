# SPDX-License-Identifier: GPL-2.0-only
"""Pinned native sessions and selected Rust closed-constant conformance tests."""

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

SOURCE: Final = Path("scripts/compat/script-vm/constants")
TESTS: Final = (
    "shared_realm_reads_previous_compilation",
    "failed_compilation_preserves_published_constant",
    "bounded_constant_rejections_are_not_lexer_policy",
)
BUILD: Final = [
    "cargo",
    "test",
    "--locked",
    "-p",
    "ottd-script",
    "--test",
    "constants",
    "--no-run",
    "--message-format=json",
]
CONTROLS: Final = {
    "publication": ("replacement-failure", "lookup K - integer 7", "lookup K - absent"),
    "counter": ("enum-counter-buffer", "lookup E D integer 1", "lookup E D integer 5"),
    "shared": ("shared-lifetime", "lookup K - absent", "lookup K - integer 7"),
    "lifetime": ("release-child", "refs owned-value 0", "refs owned-value 1"),
}


def cases() -> tuple[str, ...]:
    match read_json(ROOT / SOURCE / "witnesses.json"):
        case {"cases": list() as rows}:
            names: list[str] = []
            for row in rows:
                match row:
                    case {"id": str() as name, "session": str() as session} if (
                        session == f"fixtures/{name}.session"
                    ):
                        names.append(name)
                    case _:
                        raise WorldCheckError("Invalid constant session declaration")
        case _:
            raise WorldCheckError("Missing constant session declarations")
    if len(names) != 115 or len(set(names)) != 115:
        raise WorldCheckError("Incomplete constant native session roster")
    return tuple(names)


def input_names() -> tuple[str, ...]:
    paths = tuple(
        sorted(p.name for p in (ROOT / SOURCE / "fixtures").iterdir() if p.is_file())
    )
    if len(paths) != 152 or {p for p in paths if p.endswith(".session")} != {
        f"{name}.session" for name in cases()
    }:
        raise WorldCheckError("Constant session input membership differs")
    return paths


def argv(origin: Path, name: str) -> list[str]:
    return [
        str(origin / "native/observe"),
        "--constants-session",
        f"fixtures/{name}.session",
    ]


def observe(origin: Path, name: str) -> None:
    """Retain the real session working directory and every native process stream."""
    destination = origin / "constants/sessions" / name
    destination.mkdir(parents=True)
    arguments = argv(origin, name)
    cwd = origin / "constants/inputs"
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
        or result.stdout != (ROOT / SOURCE / "native" / f"{name}.txt").read_text()
    ):
        raise WorldCheckError(f"Native constant session differs: {name}")


def run_constants(builder: FoundationRun) -> None:
    validate_corpus()
    output = builder.output
    built = builder.run("build-constant-tests", BUILD)
    tests = select_executable(built.stdout, "constants", "test", test=True)
    target = output / "bin/constant-tests"
    _ = shutil.copy2(tests, target)
    target.chmod(0o555)
    identity: dict[str, Json] = {"path": str(tests), "sha256": digest(tests)}
    write_json(output / "constant-identities.json", identity)
    observed = run([str(tests), "--test-threads=1"], output / "constant-tests")
    require_tests(observed.stdout, TESTS)
    destination = output / "constants/inputs/fixtures"
    destination.mkdir(parents=True)
    for name in input_names():
        _ = shutil.copy2(ROOT / SOURCE / "fixtures" / name, destination / name)
    for name in cases():
        observe(output, name)
    run_controls(output)
    if digest(tests) != identity["sha256"] or digest(target) != identity["sha256"]:
        raise WorldCheckError("Constant test executable changed")
    write_json(output / "constant-identity-after.json", identity)


def validate_constants(directory: Path, origin: Path) -> None:
    validate_corpus()
    match read_json(directory / "constant-identities.json"):
        case {"path": str() as tests, "sha256": str() as sha}:
            pass
        case _:
            raise WorldCheckError("Missing constant test identity")
    if (
        read_json(directory / "constant-identity-after.json")
        != {"path": tests, "sha256": sha}
        or digest(directory / "bin/constant-tests") != sha
    ):
        raise WorldCheckError("Constant test executable changed")
    if (
        read_json(directory / "logs/build-constant-tests/argv.json") != BUILD
        or str(
            select_executable(
                (directory / "logs/build-constant-tests/stdout.log").read_text(),
                "constants",
                "test",
                test=True,
            )
        )
        != tests
    ):
        raise WorldCheckError("Constant test build selection differs")
    if read_json(directory / "constant-tests/argv.json") != [tests, "--test-threads=1"]:
        raise WorldCheckError("Constant test invocation differs")
    require_tests((directory / "constant-tests/stdout.log").read_text(), TESTS)
    for name in input_names():
        if digest(directory / "constants/inputs/fixtures" / name) != digest(
            ROOT / SOURCE / "fixtures" / name
        ):
            raise WorldCheckError("Constant session input changed")
    for name in cases():
        validate_session(directory, origin, name)
    validate_controls(directory, origin)


def validate_session(directory: Path, origin: Path, name: str) -> None:
    output = directory / "constants/sessions" / name
    if read_json(output / "argv.json") != argv(origin, name) or read_json(
        output / "cwd.json"
    ) != str(origin / "constants/inputs"):
        raise WorldCheckError("Constant session invocation differs")
    if read_json(output / "process.json") != {"returncode": 0, "expected": 0}:
        raise WorldCheckError("Constant session process failed")
    if (output / "stderr.log").read_text():
        raise WorldCheckError("Constant session stderr differs")
    if (output / "stdout.log").read_text() != (
        ROOT / SOURCE / "native" / f"{name}.txt"
    ).read_text():
        raise ConstantObservationMismatchError("Constant session observation differs")


def validate_corpus() -> None:
    """The private Rust corpus must consume the same original buffer captures."""
    fixture_names = [name for name in input_names() if name.endswith(".nut")]
    rust = ROOT / "crates/ottd-script/tests/constants"
    for name in fixture_names:
        if (rust / "fixtures" / name).read_bytes() != (
            ROOT / SOURCE / "fixtures" / name
        ).read_bytes():
            raise WorldCheckError("Rust constant source fixture differs")
        stem = Path(name).stem
        if (rust / "native" / f"{stem}.txt").read_bytes() != (
            ROOT / SOURCE / "native" / f"{stem}-buffer.txt"
        ).read_bytes():
            raise WorldCheckError("Rust constant buffer capture differs")
    for name in ("shared-lifetime", "replacement-failure", "updates"):
        if (rust / "native" / f"{name}-session.txt").read_bytes() != (
            ROOT / SOURCE / "native" / f"{name}.txt"
        ).read_bytes() or (rust / "native" / f"{name}.session").read_bytes() != (
            ROOT / SOURCE / "fixtures" / f"{name}.session"
        ).read_bytes():
            raise WorldCheckError("Rust shared constant session capture differs")


class ConstantObservationMismatchError(WorldCheckError):
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
        "scripts.script_vm_constants",
        "--validate-session",
        str(directory),
        str(origin),
        name,
    ]


def admission_text(name: str, *, rejected: bool) -> str:
    return (
        f"REJECT constant session observation differs: {name}\n"
        if rejected
        else f"PASS constant session admission: {name}\n"
    )


def run_controls(output: Path) -> None:
    for name, (session, old, new) in CONTROLS.items():
        control = output / "controls" / f"constant-{name}"
        original = output / "constants/sessions" / session
        accepted = run(
            admission_argv(output, output, session), control / "admit-original"
        )
        if (
            accepted.stdout != admission_text(session, rejected=False)
            or accepted.stderr
        ):
            raise WorldCheckError("Original constant session was not admitted")
        mutant = control / "mutant"
        copied = mutant / "constants/sessions" / session
        copied.mkdir(parents=True)
        before: dict[str, Json] = {}
        for filename in SESSION_FILES:
            before[filename] = digest(original / filename)
            _ = shutil.copy2(original / filename, copied / filename)
        source = (original / "stdout.log").read_text()
        if old not in source:
            raise WorldCheckError("Missing constant mutation target")
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
            raise WorldCheckError(
                "Constant stdout mutation lacked its specific refusal"
            )
        for filename in SESSION_FILES:
            if digest(original / filename) != before[filename]:
                raise WorldCheckError(
                    "Original constant capture changed during control"
                )
    validate_controls(output, output)


def validate_controls(directory: Path, origin: Path) -> None:
    for name, (session, old, new) in CONTROLS.items():
        control = directory / "controls" / f"constant-{name}"
        original = directory / "constants/sessions" / session
        mutant = control / "mutant"
        copied = mutant / "constants/sessions" / session
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
            raise WorldCheckError("Constant mutation source bindings differ")
        for filename in SESSION_FILES:
            expected = (original / filename).read_bytes()
            if filename == "stdout.log":
                if old not in expected.decode():
                    raise WorldCheckError("Constant control target missing")
                expected = expected.decode().replace(old, new).encode()
            if (copied / filename).read_bytes() != expected:
                raise WorldCheckError(
                    "Constant control changed more than its declared stdout"
                )
        try:
            validate_session(mutant, origin, session)
        except ConstantObservationMismatchError:
            pass
        else:
            raise WorldCheckError("Constant stdout mutation was admitted")
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
            raise WorldCheckError("Constant control admission receipt differs")


def main() -> int:
    match sys.argv[1:]:
        case ["--validate-session", directory, origin, name]:
            try:
                validate_session(Path(directory), Path(origin), name)
            except ConstantObservationMismatchError:
                print(admission_text(name, rejected=True), end="")
                return 1
            except WorldCheckError as error:
                print(f"ERROR constant session validation: {error}", file=sys.stderr)
                return 2
            print(admission_text(name, rejected=False), end="")
            return 0
        case _:
            raise WorldCheckError("Expected --validate-session DIRECTORY ORIGIN NAME")


if __name__ == "__main__":
    sys.exit(main())
