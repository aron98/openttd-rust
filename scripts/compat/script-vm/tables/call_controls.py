"""Real-process corruption controls for raw-safe call evidence publication."""

import json
import shutil
import sys
from pathlib import Path
from typing import Literal

from .calls import audit, invoke, print_payloads
from .case import Case
from .compare import BUDGETS, digest

Mutation = Literal[
    "nul",
    "order",
    "framing",
    "stage",
    "error",
    "budget",
    "status",
    "status-bool",
    "missing",
]


def corrupt(output: Path, mutation: Mutation) -> None:
    prefix = output / "control.0-0-rust"
    match mutation:
        case "nul":
            payload = b"A\0B"
            file = Path(f"{prefix}.print")
            data = file.read_bytes()
            if payload not in data:
                raise RuntimeError("NUL control prerequisite missing")
            _ = file.write_bytes(data.replace(payload, b"AXB"))
        case "order":
            file = Path(f"{prefix}.print")
            payloads = print_payloads(file.read_bytes())
            _ = file.write_bytes(
                b"".join(
                    len(part).to_bytes(8, "little") + part
                    for part in reversed(payloads)
                )
            )
        case "framing":
            file = Path(f"{prefix}.print")
            _ = file.write_bytes(file.read_bytes()[:-1])
        case "stage":
            _ = Path(f"{prefix}.stdout").write_bytes(b"compile_error\n")
        case "error":
            _ = Path(f"{prefix}.stderr").write_bytes(b"wrong number of parameters\n")
        case "budget" | "status" | "status-bool":
            # Reconstruct this known real invocation and mutate one semantic field.
            argv = [str(output / "rust"), str(output / "control.0.nut"), "10000"]
            if mutation == "budget":
                argv[-1] = "9999"
            receipt = {
                "argv": argv,
                "status": False
                if mutation == "status-bool"
                else (1 if mutation == "status" else 0),
                "print_file": f"{prefix}.print",
                "sha256": {
                    ext: digest(Path(f"{prefix}.{ext}"))
                    for ext in ("stdout", "stderr", "print")
                },
            }
            _ = Path(f"{prefix}.json").write_text(json.dumps(receipt))
            return
        case "missing":
            Path(f"{prefix}.stdout").unlink()
            return
    # Updating the hash prevents a trivial stale-checksum rejection from masking
    # failures of strict raw comparison, framing, or expected-stage admission.
    receipt = {
        "argv": [str(output / "rust"), str(output / "control.0.nut"), "10000"],
        "status": 0,
        "print_file": f"{prefix}.print",
        "sha256": {
            ext: digest(Path(f"{prefix}.{ext}"))
            for ext in ("stdout", "stderr", "print")
        },
    }
    _ = Path(f"{prefix}.json").write_text(json.dumps(receipt))


def controls(native: Path, rust: Path, output: Path) -> None:
    case = Case(
        "control.0", 'print("A\\0B");\nprint("C");\nassert(false);', "runtime_error"
    )
    mutations: tuple[Mutation, ...] = (
        "nul",
        "order",
        "framing",
        "stage",
        "error",
        "budget",
        "status",
        "status-bool",
        "missing",
    )
    output.mkdir(parents=True, exist_ok=False)
    for mutation in mutations:
        attempt = output / mutation
        attempt.mkdir()
        source = attempt / f"{case.name}.nut"
        _ = source.write_text(case.source)
        for side, binary in (("native", native), ("rust", rust)):
            _ = shutil.copy2(binary, attempt / side)
        for number, budget in enumerate(BUDGETS):
            for side in ("native", "rust"):
                invoke(
                    [str(attempt / side), str(source), *map(str, budget)],
                    attempt / f"{case.name}-{number}-{side}",
                )
        _ = audit(attempt, [case])
        corrupt(attempt, mutation)
        try:
            _ = audit(attempt, [case])
        except (RuntimeError, FileNotFoundError) as error:
            _ = (attempt / "rejection.txt").write_text(str(error) + "\n")
        else:
            raise RuntimeError(f"Corruption accepted: {mutation}")
    print(f"PASS {len(mutations)} real-process call publication corruptions")


if __name__ == "__main__":
    controls(*(Path(argument).resolve() for argument in sys.argv[1:]))
