"""Execute and retain strict original-versus-Rust table scenarios."""

import hashlib
import json
import shutil
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path

from .cases import Case, cases


@dataclass(frozen=True, slots=True)
class Invocation:
    argv: list[str]
    status: int
    stdout_sha256: str
    stderr_sha256: str


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(argv: list[str], prefix: Path) -> bytes:
    completed = subprocess.run(argv, capture_output=True, check=False, timeout=60)
    stdout = prefix.with_suffix(".stdout")
    stderr = prefix.with_suffix(".stderr")
    _ = stdout.write_bytes(completed.stdout)
    _ = stderr.write_bytes(completed.stderr)
    receipt = Invocation(argv, completed.returncode, digest(stdout), digest(stderr))
    _ = prefix.with_suffix(".json").write_text(json.dumps(asdict(receipt), indent=2))
    if completed.returncode != 0:
        raise RuntimeError(f"Process failed: {prefix}")
    return completed.stdout


def validate(case: Case, native: bytes, rust: bytes) -> None:
    if native != rust:
        raise RuntimeError(f"Strict native/Rust mismatch: {case.name}")
    if not any(
        line.startswith(case.stage.encode() + b" ") or line == case.stage.encode()
        for line in native.splitlines()
    ):
        raise RuntimeError(f"Expected {case.stage}: {case.name}")


def compare(native: Path, rust: Path, output: Path) -> None:
    output.mkdir(parents=True, exist_ok=False)
    for name, binary in [("native", native), ("rust", rust)]:
        _ = shutil.copy2(binary, output / name)
    records: list[dict[str, str | int]] = []
    for case in cases():
        source = output / f"{case.name}.nut"
        _ = source.write_text(case.source)
        for number, budget in enumerate(
            [[10000], [0, 1, 2, 4, 0, 8, 16, 32, 10000], [2] * 40 + [10000]]
        ):
            argv = [str(source), *map(str, budget)]
            left = run(
                [str(output / "native"), *argv], output / f"{case.name}-{number}-native"
            )
            right = run(
                [str(output / "rust"), *argv], output / f"{case.name}-{number}-rust"
            )
            validate(case, left, right)
            records.append(
                {
                    "case": case.name,
                    "budget": number,
                    "source_sha256": digest(source),
                    "stdout_sha256": hashlib.sha256(left).hexdigest(),
                }
            )
    manifest = {
        "native_sha256": digest(output / "native"),
        "rust_sha256": digest(output / "rust"),
        "records": records,
    }
    _ = (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"PASS {len(records)} strict native/Rust table scenarios")


if __name__ == "__main__":
    compare(*(Path(arg).resolve() for arg in sys.argv[1:]))
