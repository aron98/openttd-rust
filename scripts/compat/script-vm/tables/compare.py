"""Execute and retain strict original-versus-Rust table scenarios."""

import hashlib
import json
import re
import shutil
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Final

from scripts.world_check_support import read_json

from .case import Case
from .cases import cases


@dataclass(frozen=True, slots=True)
class Invocation:
    argv: list[str]
    status: int
    stdout_sha256: str
    stderr_sha256: str


@dataclass(frozen=True, slots=True)
class Observation:
    case: str
    budget: int
    credits: tuple[int, ...]
    source: str
    native_prefix: str
    rust_prefix: str
    source_sha256: str
    stdout_sha256: str


BUDGETS: Final = ((10000,), (0, 1, 2, 4, 0, 8, 16, 32, 10000), (*([2] * 40), 10000))


def source_arguments(case: Case, source: Path, budget: tuple[int, ...]) -> list[str]:
    return [str(source), *(["--errors"] if case.error_trace else []), *map(str, budget)]


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(argv: list[str], prefix: Path) -> bytes:
    completed = subprocess.run(argv, capture_output=True, check=False, timeout=60)
    stdout = prefix.parent / f"{prefix.name}.stdout"
    stderr = prefix.parent / f"{prefix.name}.stderr"
    _ = stdout.write_bytes(completed.stdout)
    _ = stderr.write_bytes(completed.stderr)
    receipt = Invocation(argv, completed.returncode, digest(stdout), digest(stderr))
    _ = (prefix.parent / f"{prefix.name}.json").write_text(
        json.dumps(asdict(receipt), indent=2)
    )
    if completed.returncode != 0:
        raise RuntimeError(f"Process failed: {prefix}")
    return completed.stdout


def validate_error_value(line: bytes) -> None:
    parts = line.split(b" ", 3)
    if parts == [b"last_error", b"null"]:
        return
    try:
        if len(parts) == 4 and parts[1] == b"string":
            size = int(parts[2])
            if (
                size >= 0
                and len(parts[3]) == size * 2
                and re.fullmatch(rb"[0-9a-f]*", parts[3])
            ):
                return
        if len(parts) == 3:
            value = int(parts[2])
            limits = {
                b"integer": (-(1 << 63), (1 << 63) - 1),
                b"float": (0, (1 << 32) - 1),
                b"bool": (0, 1),
            }
            if parts[1] in limits:
                low, high = limits[parts[1]]
                if low <= value <= high and str(value).encode() == parts[2]:
                    return
            if parts[1] == b"unsupported" and value in {
                134217984,
                134218240,
                167772192,
                134217792,
            }:
                return
    except ValueError as error:
        raise RuntimeError("Malformed native error object") from error
    raise RuntimeError("Malformed native error object")


def validate_error_trace(case: Case, output: bytes) -> None:
    if not case.error_trace:
        return
    lines = output.splitlines()
    states = [
        i
        for i, line in enumerate(lines)
        if line == b"compile_error"
        or line.startswith((b"suspend ", b"return ", b"runtime_error "))
    ]
    snapshots = [i for i, line in enumerate(lines) if line.startswith(b"last_error ")]
    expected = [i + 1 if lines[i] == b"compile_error" else i - 1 for i in states]
    if not states or snapshots != expected:
        raise RuntimeError("Missing or misplaced native error object channel")
    for position in snapshots:
        validate_error_value(lines[position])


def validate(case: Case, native: bytes, rust: bytes) -> None:
    if native != rust:
        raise RuntimeError(f"Strict native/Rust mismatch: {case.name}")
    validate_error_trace(case, native)
    if not any(
        line.startswith(case.stage.encode() + b" ") or line == case.stage.encode()
        for line in native.splitlines()
    ):
        raise RuntimeError(f"Expected {case.stage}: {case.name}")


def validate_runtime_stderr(case: Case, native: bytes, rust: bytes) -> None:
    """Compare full raw compiler and runtime diagnostic channels."""
    if native != rust:
        raise RuntimeError(f"Strict native/Rust stderr mismatch: {case.name}")


def verify_retention(
    output: Path, scenarios: list[Case], records: list[Observation]
) -> None:
    """Re-read complete unique inputs and receipts before publishing success."""
    expected = {
        (case.name, number): (case, budget)
        for case in scenarios
        for number, budget in enumerate(BUDGETS)
    }
    if len({case.name for case in scenarios}) != len(scenarios):
        raise RuntimeError("Duplicate source identity")
    if len(records) != len(expected) or {(r.case, r.budget) for r in records} != set(
        expected
    ):
        raise RuntimeError("Duplicate or missing observation identity")
    paths = {"native", "rust"} | {f"{case.name}.nut" for case in scenarios}
    for case, number in expected:
        for side in ("native", "rust"):
            paths.update(
                f"{case}-{number}-{side}.{ext}" for ext in ("stdout", "stderr", "json")
            )
    actual = {path.name for path in output.iterdir() if path.is_file()}
    if actual != paths:
        raise RuntimeError("Retained artifact membership differs")
    for record in records:
        verify_observation(output, record, expected[(record.case, record.budget)])


def verify_observation(
    output: Path, record: Observation, expected: tuple[Case, tuple[int, ...]]
) -> None:
    case, budget = expected
    source = output / f"{case.name}.nut"
    if (
        record.source != source.name
        or record.credits != budget
        or source.read_bytes() != case.source.encode()
        or record.source_sha256 != digest(source)
    ):
        raise RuntimeError("Retained source or budget differs")
    values: list[bytes] = []
    errors: list[bytes] = []
    for side, prefix in (
        ("native", record.native_prefix),
        ("rust", record.rust_prefix),
    ):
        name = f"{case.name}-{record.budget}-{side}"
        if prefix != name:
            raise RuntimeError("Retained process identity differs")
        stdout = output / f"{name}.stdout"
        stderr = output / f"{name}.stderr"
        receipt = read_json(output / f"{name}.json")
        expected_receipt = {
            "argv": [str(output / side), *source_arguments(case, source, budget)],
            "status": 0,
            "stdout_sha256": digest(stdout),
            "stderr_sha256": digest(stderr),
        }
        if json.dumps(receipt, sort_keys=True) != json.dumps(
            expected_receipt, sort_keys=True
        ):
            raise RuntimeError("Retained process receipt differs")
        if record.stdout_sha256 != digest(stdout):
            raise RuntimeError("Retained result differs")
        values.append(stdout.read_bytes())
        errors.append(stderr.read_bytes())
    validate(case, values[0], values[1])
    validate_runtime_stderr(case, errors[0], errors[1])


def compare(native: Path, rust: Path, output: Path) -> None:
    output.mkdir(parents=True, exist_ok=False)
    for name, binary in [("native", native), ("rust", rust)]:
        _ = shutil.copy2(binary, output / name)
    records: list[Observation] = []
    scenarios = cases()
    for case in scenarios:
        source = output / f"{case.name}.nut"
        _ = source.write_text(case.source)
        for number, budget in enumerate(BUDGETS):
            argv = source_arguments(case, source, budget)
            left = run(
                [str(output / "native"), *argv], output / f"{case.name}-{number}-native"
            )
            right = run(
                [str(output / "rust"), *argv], output / f"{case.name}-{number}-rust"
            )
            validate(case, left, right)
            validate_runtime_stderr(
                case,
                (output / f"{case.name}-{number}-native.stderr").read_bytes(),
                (output / f"{case.name}-{number}-rust.stderr").read_bytes(),
            )
            records.append(
                Observation(
                    case.name,
                    number,
                    budget,
                    source.name,
                    f"{case.name}-{number}-native",
                    f"{case.name}-{number}-rust",
                    digest(source),
                    hashlib.sha256(left).hexdigest(),
                )
            )
    verify_retention(output, scenarios, records)
    manifest = {
        "native_sha256": digest(output / "native"),
        "rust_sha256": digest(output / "rust"),
        "records": [asdict(record) for record in records],
        "retention_verified": True,
    }
    _ = (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"PASS {len(records)} strict native/Rust table scenarios")


if __name__ == "__main__":
    compare(*(Path(arg).resolve() for arg in sys.argv[1:]))
