"""Fresh original/Rust closure traces must reject recomputed-hash corruptions."""

import json
import os
import shutil
import sys
from pathlib import Path

from .calls import audit, invoke
from .case import Case
from .closure_cases import joined_source
from .compare import BUDGETS, digest


def controls(native: Path, rust: Path, output: Path) -> None:
    os.environ["OTTD_SCRIPT_CLOSURE_TRACE"] = "1"
    case = Case(
        "closure.0",
        joined_source(
            'local x=7;\nlocal f=function():(x){print("A\\0B");\n',
            "assert(false);\nreturn x;};\nreturn f();",
        ),
        "runtime_error",
    )
    output.mkdir(parents=True, exist_ok=False)
    for marker in (
        b"outer main.0 ",
        b"source main.0 ",
        b"child-op main.0 ",
        b"suspend ",
    ):
        name = marker.decode().strip().replace(" ", "-")
        attempt = output / name
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
        prefix = attempt / f"{case.name}-2-rust"
        raw = Path(f"{prefix}.stdout")
        lines = raw.read_bytes().splitlines(keepends=True)
        index = next(i for i, line in enumerate(lines) if line.startswith(marker))
        lines[index] = lines[index].rstrip(b"\n") + b" changed\n"
        _ = raw.write_bytes(b"".join(lines))
        receipt = {
            "argv": [str(attempt / "rust"), str(source), *map(str, BUDGETS[2])],
            "status": 0,
            "print_file": f"{prefix}.print",
            "sha256": {
                extension: digest(Path(f"{prefix}.{extension}"))
                for extension in ("stdout", "stderr", "print")
            },
        }
        _ = Path(f"{prefix}.json").write_text(json.dumps(receipt))
        try:
            _ = audit(attempt, [case])
        except RuntimeError as error:
            _ = (attempt / "rejection.txt").write_text(str(error) + "\n")
        else:
            raise RuntimeError(f"Closure corruption accepted: {name}")
    print("PASS 4 real-process closure metadata/frame corruptions")


if __name__ == "__main__":
    controls(*(Path(argument).resolve() for argument in sys.argv[1:]))
