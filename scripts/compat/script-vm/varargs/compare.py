# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 -m scripts.compat.script-vm.varargs.compare NATIVE RUST OUTPUT
"""Compare whole variadic traces with exact retained invocation inventories."""

import json
import os
import shutil
import sys
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Final

from scripts.world_check_support import read_json

from ..tables.calls import invoke  # noqa: TID252 - script-vm is not an identifier.
from ..tables.compare import digest  # noqa: TID252 - script-vm is not an identifier.
from .case import Case
from .cases import CASES

BUDGETS: Final = (
    (10000,),
    (0, 1, 2, 4, 0, 8, 16, 32, 10000),
    (0,) + (2,) * 200 + (10000,),
)


@dataclass(frozen=True, slots=True)
class Record:
    case: str
    budget: int
    source_sha256: str
    stdout_sha256: str
    print_sha256: str


def terminal(case: Case, stdout: bytes, stderr: bytes) -> None:
    """Require the independently specified original terminal result."""
    terminals = [
        line
        for line in stdout.splitlines()
        if line.startswith((b"return ", b"runtime_error ", b"compile_error"))
    ]
    if len(terminals) != 1 or terminals[0].split(b" ")[0] != case.stage.encode():
        raise RuntimeError(f"Unexpected native stage: {case.name}")
    outcome = terminals[0] if case.stage == "return" else stderr.rstrip(b"\n")
    if not outcome.endswith(case.outcome):
        raise RuntimeError(f"Unexpected native outcome: {case.name}: {outcome!r}")


def audit(output: Path, *, recorded: Path | None = None) -> list[Record]:
    """Read actual records back and compare full output, errors and prints."""
    recorded = output if recorded is None else recorded
    expected = {"native", "rust"}
    records: list[Record] = []
    for case in CASES:
        source = output / f"{case.name}.nut"
        expected.add(source.name)
        if source.read_text() != case.source:
            raise RuntimeError(f"Changed source: {case.name}")
        for number, base_budget in enumerate(BUDGETS):
            budgets = (*base_budget, *case.budget_extension)
            results: dict[str, dict[str, bytes]] = {}
            for side in ("native", "rust"):
                prefix = f"{case.name}-{number}-{side}"
                files = {
                    ext: output / f"{prefix}.{ext}"
                    for ext in ("stdout", "stderr", "print", "json")
                }
                expected.update(p.name for p in files.values())
                receipt = {
                    "argv": [
                        str(recorded / side),
                        str(recorded / source.name),
                        *map(str, budgets),
                    ],
                    "status": 0,
                    "print_file": str(recorded / files["print"].name),
                    "sha256": {
                        ext: digest(files[ext]) for ext in ("stdout", "stderr", "print")
                    },
                }
                if json.dumps(read_json(files["json"]), sort_keys=True) != json.dumps(
                    receipt, sort_keys=True
                ):
                    raise RuntimeError(f"Receipt mismatch: {prefix}")
                results[side] = {
                    ext: files[ext].read_bytes()
                    for ext in ("stdout", "stderr", "print")
                }
            native, rust = results["native"], results["rust"]
            terminal(case, native["stdout"], native["stderr"])
            if native["stdout"] != rust["stdout"] or native["print"] != rust["print"]:
                raise RuntimeError(f"Full trace mismatch: {case.name}, budget {number}")
            if case.stage != "compile_error" and native["stderr"] != rust["stderr"]:
                raise RuntimeError(f"Runtime error mismatch: {case.name}")
            records.append(
                Record(
                    case.name,
                    number,
                    digest(source),
                    digest(output / f"{case.name}-{number}-native.stdout"),
                    digest(output / f"{case.name}-{number}-native.print"),
                )
            )
    if {p.name for p in output.iterdir()} != expected:
        raise RuntimeError("Unexpected retained member inventory")
    return records


def compare(native: Path, rust: Path, output: Path) -> None:
    """Execute only the explicit defined-source corpus; refuse reused output."""
    if len({case.name for case in CASES}) != len(CASES):
        raise RuntimeError("Duplicate case names")
    output.mkdir(parents=True, exist_ok=False)
    os.environ["OTTD_SCRIPT_CLOSURE_TRACE"] = "1"
    os.environ["OTTD_SCRIPT_VARARGS_TRACE"] = "1"
    for side, binary in (("native", native), ("rust", rust)):
        _ = shutil.copy2(binary, output / side)
    for case in CASES:
        source = output / f"{case.name}.nut"
        _ = source.write_text(case.source)
        for number, base_budget in enumerate(BUDGETS):
            budgets = (*base_budget, *case.budget_extension)
            for side in ("native", "rust"):
                invoke(
                    [str(output / side), str(source), *map(str, budgets)],
                    output / f"{case.name}-{number}-{side}",
                )
    records = audit(output)
    _ = (output / "manifest.json").write_text(
        json.dumps(
            {
                "native_sha256": digest(output / "native"),
                "rust_sha256": digest(output / "rust"),
                "environment": {
                    "OTTD_SCRIPT_CLOSURE_TRACE": "1",
                    "OTTD_SCRIPT_VARARGS_TRACE": "1",
                },
                "records": [asdict(record) for record in records],
                "retention_verified": True,
            },
            indent=2,
        )
        + "\n"
    )
    print(f"PASS {len(records)} whole variadic trace comparisons")


if __name__ == "__main__":
    compare(*(Path(arg).resolve() for arg in sys.argv[1:]))
