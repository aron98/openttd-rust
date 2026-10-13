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

from ..tables.compare import digest  # noqa: TID252 - script-vm is not an identifier.
from .compare import audit

Mutation = Literal["missing", "source", "stdout", "receipt", "compiler-stderr"]


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
        case "compiler-stderr":
            path = copied / "trailing-comma-0-rust.stderr"
            original = path.read_bytes()
            if not original:
                raise RuntimeError("Missing genuine compiler diagnostic")
            _ = path.write_bytes(original + b"compiler corruption\n")
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
        "compiler-stderr",
    )
    with tempfile.TemporaryDirectory(prefix="varargs-audit-") as temporary:
        copied = Path(temporary) / "copied"
        _ = shutil.copytree(capture, copied)
        (copied / "manifest.json").unlink()
        records = audit(copied, recorded=capture)
        results.append(f"baseline {len(records)} PASS")
        for mutation in mutations:
            path = change(copied, mutation)
            receipt_path = copied / "trailing-comma-0-rust.json"
            if mutation == "compiler-stderr":
                prefix = capture / "trailing-comma-0-rust"
                _ = receipt_path.write_text(
                    json.dumps(
                        {
                            "argv": [
                                str(capture / "rust"),
                                str(capture / "trailing-comma.nut"),
                                "10000",
                            ],
                            "status": 0,
                            "print_file": str(prefix) + ".print",
                            "sha256": {
                                ext: digest(copied / ("trailing-comma-0-rust." + ext))
                                for ext in ("stdout", "stderr", "print")
                            },
                        }
                    )
                )
                _ = (output / "compiler-stderr.stderr").write_bytes(path.read_bytes())
                _ = (output / "compiler-stderr.json").write_bytes(
                    receipt_path.read_bytes()
                )
            try:
                _ = audit(copied, recorded=capture)
            except (RuntimeError, FileNotFoundError) as error:
                results.append(f"{mutation} REJECT {type(error).__name__}: {error}")
            else:
                raise RuntimeError(f"Corruption accepted: {mutation}")
            finally:
                _ = shutil.copy2(capture / path.name, path)
                if mutation == "compiler-stderr":
                    _ = shutil.copy2(capture / receipt_path.name, receipt_path)
        if len(audit(copied, recorded=capture)) != len(records):
            raise RuntimeError("Restored baseline differs")
    _ = (output / "result.txt").write_text("\n".join(results) + "\n")
    print("PASS five corruptions rejected and copied baseline restored")


if __name__ == "__main__":
    controls(*(Path(arg).resolve() for arg in sys.argv[1:]))
