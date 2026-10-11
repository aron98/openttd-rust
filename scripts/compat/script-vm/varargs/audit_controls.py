# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 -m scripts.compat.script-vm.varargs.audit_controls CAPTURE OUTPUT
"""Exercise rejection of actual retained payload and invocation corruption."""

import json
import shutil
import sys
import tempfile
from pathlib import Path
from typing import Literal, assert_never

from .compare import audit

Mutation = Literal["missing", "source", "stdout", "receipt"]


def change(copied: Path, mutation: Mutation) -> Path:
    """Apply a single corruption to a private copy and identify its path."""
    match mutation:
        case "missing":
            path = copied / "discovery-0-native.json"
            path.unlink()
            return path
        case "source":
            path = copied / "discovery.nut"
            _ = path.write_text("return 999;")
            return path
        case "stdout":
            path = copied / "discovery-0-native.stdout"
            _ = path.write_bytes(b"return 0 integer 999\n")
            return path
        case "receipt":
            path = copied / "discovery-0-native.json"
            _ = path.write_text(json.dumps({"status": True}))
            return path
    assert_never(mutation)


def controls(capture: Path, output: Path) -> None:
    output.mkdir(parents=True, exist_ok=False)
    results: list[str] = []
    mutations: tuple[Mutation, ...] = (
        "missing",
        "source",
        "stdout",
        "receipt",
    )
    with tempfile.TemporaryDirectory(prefix="varargs-audit-") as temporary:
        copied = Path(temporary) / "copied"
        _ = shutil.copytree(capture, copied)
        (copied / "manifest.json").unlink()
        records = audit(copied, recorded=capture)
        results.append(f"baseline {len(records)} PASS")
        for mutation in mutations:
            path = change(copied, mutation)
            try:
                _ = audit(copied, recorded=capture)
            except (RuntimeError, FileNotFoundError) as error:
                results.append(f"{mutation} REJECT {type(error).__name__}: {error}")
            else:
                raise RuntimeError(f"Corruption accepted: {mutation}")
            finally:
                _ = shutil.copy2(capture / path.name, path)
        if len(audit(copied, recorded=capture)) != len(records):
            raise RuntimeError("Restored baseline differs")
    _ = (output / "result.txt").write_text("\n".join(results) + "\n")
    print("PASS four corruptions rejected and copied baseline restored")


if __name__ == "__main__":
    controls(*(Path(arg).resolve() for arg in sys.argv[1:]))
