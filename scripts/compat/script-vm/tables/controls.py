"""Require real native capture corruptions to fail the production comparator."""

import json
import sys
from pathlib import Path

from .case import Case
from .compare import validate


def controls(output: Path) -> None:
    native = (output / "empty-1-native.stdout").read_bytes()
    source = (output / "empty.nut").read_text()
    case = Case("empty", source)
    variants = {
        "opcode": native.replace(b"op 30 ", b"op 31 ", 1),
        "result": native.replace(b"7461626c65", b"6172726179", 1),
        "debt": native.replace(b"suspend -1 ", b"suspend -2 ", 1),
        "cursor": native.replace(b"suspend -1 0", b"suspend -1 1", 1),
        "vacuous-compilation": b"compile_error\n",
    }
    rejected: list[str] = []
    for name, changed in variants.items():
        if changed == native:
            raise RuntimeError(f"Control did not change its input: {name}")
        _ = (output / f"control-{name}.stdout").write_bytes(changed)
        left = changed if name == "vacuous-compilation" else native
        try:
            validate(case, left, changed)
        except RuntimeError as error:
            _ = (output / f"control-{name}.error").write_text(str(error))
            rejected.append(name)
        else:
            raise RuntimeError(f"Comparator accepted corruption: {name}")
    _ = (output / "controls.json").write_text(json.dumps(rejected) + "\n")
    print(f"PASS rejected {len(rejected)} native observation corruptions")


if __name__ == "__main__":
    controls(Path(sys.argv[1]))
