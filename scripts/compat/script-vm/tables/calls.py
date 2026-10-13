"""Preserve raw print callbacks outside execution records and audit every receipt."""

import hashlib
import json
import os
import shutil
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path

from scripts.world_check_support import read_json

from .call_cases import cases
from .case import Case
from .compare import BUDGETS, digest, validate


@dataclass(frozen=True, slots=True)
class Record:
    case: str
    budget: int
    source_sha256: str
    stdout_sha256: str
    print_sha256: str


def invoke(argv: list[str], prefix: Path) -> None:
    print_file = prefix.parent / f"{prefix.name}.print"
    completed = subprocess.run(
        argv,
        env={**os.environ, "OTTD_SCRIPT_PRINT_FILE": str(print_file)},
        capture_output=True,
        check=False,
        timeout=60,
    )
    for extension, data in (("stdout", completed.stdout), ("stderr", completed.stderr)):
        _ = (prefix.parent / f"{prefix.name}.{extension}").write_bytes(data)
    receipt = {
        "argv": argv,
        "status": completed.returncode,
        "print_file": str(print_file),
        "sha256": {
            extension: digest(prefix.parent / f"{prefix.name}.{extension}")
            for extension in ("stdout", "stderr", "print")
        },
    }
    _ = (prefix.parent / f"{prefix.name}.json").write_text(
        json.dumps(receipt, indent=2)
    )
    if completed.returncode:
        raise RuntimeError(f"Call observer failed: {prefix}")


def print_payloads(data: bytes) -> list[bytes]:
    payloads: list[bytes] = []
    offset = 0
    while offset < len(data):
        if len(data) - offset < 8:
            raise RuntimeError("Truncated print length")
        length = int.from_bytes(data[offset : offset + 8], "little")
        offset += 8
        if length > len(data) - offset:
            raise RuntimeError("Truncated print payload")
        payloads.append(data[offset : offset + length])
        offset += length
    return payloads


def audit(output: Path, scenarios: list[Case]) -> list[Record]:
    if len({case.name for case in scenarios}) != len(scenarios):
        raise RuntimeError("Duplicate call case identity")
    expected = {"native", "rust"}
    records: list[Record] = []
    for case in scenarios:
        source = output / f"{case.name}.nut"
        expected.add(source.name)
        if source.read_bytes() != case.source.encode():
            raise RuntimeError(f"Call source mismatch: {case.name}")
        for number, budget in enumerate(BUDGETS):
            channels: dict[str, dict[str, bytes]] = {}
            for side in ("native", "rust"):
                prefix = f"{case.name}-{number}-{side}"
                paths = {
                    extension: output / f"{prefix}.{extension}"
                    for extension in ("stdout", "stderr", "print", "json")
                }
                expected.update(path.name for path in paths.values())
                # Compare the complete parsed JSON value with an independently
                # reconstructed receipt, including absolute argv and budget order.
                receipt = read_json(paths["json"])
                expected_receipt = {
                    "argv": [str(output / side), str(source), *map(str, budget)],
                    "status": 0,
                    "print_file": str(paths["print"]),
                    "sha256": {
                        extension: digest(paths[extension])
                        for extension in ("stdout", "stderr", "print")
                    },
                }
                if json.dumps(receipt, sort_keys=True) != json.dumps(
                    expected_receipt, sort_keys=True
                ):
                    raise RuntimeError(f"Call receipt mismatch: {prefix}")
                channels[side] = {
                    extension: paths[extension].read_bytes()
                    for extension in ("stdout", "stderr", "print")
                }
                _ = print_payloads(channels[side]["print"])
            native, rust = channels["native"], channels["rust"]
            validate(case, native["stdout"], rust["stdout"])
            if native["print"] != rust["print"]:
                raise RuntimeError(f"Print callback mismatch: {case.name}")
            if native["stderr"] != rust["stderr"]:
                raise RuntimeError(f"Native call error mismatch: {case.name}")
            records.append(
                Record(
                    case.name,
                    number,
                    digest(source),
                    hashlib.sha256(native["stdout"]).hexdigest(),
                    hashlib.sha256(native["print"]).hexdigest(),
                )
            )
    actual = {path.name for path in output.iterdir()}
    if actual != expected:
        raise RuntimeError("Call receipt membership differs")
    return records


def compare_scenarios(
    native: Path,
    rust: Path,
    output: Path,
    scenarios: list[Case],
    required_prefix: bytes = b"",
) -> None:
    output.mkdir(parents=True, exist_ok=False)
    for side, binary in (("native", native), ("rust", rust)):
        _ = shutil.copy2(binary, output / side)
    for case in scenarios:
        source = output / f"{case.name}.nut"
        _ = source.write_bytes(case.source.encode())
        for number, budget in enumerate(BUDGETS):
            for side in ("native", "rust"):
                invoke(
                    [str(output / side), str(source), *map(str, budget)],
                    output / f"{case.name}-{number}-{side}",
                )
    records = audit(output, scenarios)
    for case in scenarios:
        if case.stage == "compile_error":
            continue
        for number in range(len(BUDGETS)):
            for side in ("native", "rust"):
                raw = output / f"{case.name}-{number}-{side}.stdout"
                if not raw.read_bytes().startswith(required_prefix):
                    raise RuntimeError(f"Missing required observer records: {raw}")
    _ = (output / "manifest.json").write_text(
        json.dumps(
            {
                "native_sha256": digest(output / "native"),
                "rust_sha256": digest(output / "rust"),
                "records": [asdict(record) for record in records],
                "retention_verified": True,
            },
            indent=2,
        )
        + "\n"
    )
    print(f"PASS {len(records)} raw-safe native call comparisons")


def compare(native: Path, rust: Path, output: Path) -> None:
    compare_scenarios(native, rust, output, cases())


if __name__ == "__main__":
    compare(*(Path(argument).resolve() for argument in sys.argv[1:]))
