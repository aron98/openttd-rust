# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 -m scripts.compat.script-vm.varargs.owners NATIVE OUTPUT
"""Retain actual defined owner invocations for Rust's complete trace consumers."""

import json
import shutil
import subprocess
import sys
from pathlib import Path

from ..tables.compare import digest  # noqa: TID252 - script-vm is not an identifier.


def capture(native: Path, output: Path) -> None:
    output.mkdir(parents=True, exist_ok=False)
    binary = output / "native"
    _ = shutil.copy2(native, binary)
    for name in ("return", "tail", "error"):
        source = output / f"owner-{name}.nut"
        _ = shutil.copy2(Path(__file__).parent / "fixtures" / source.name, source)
        argv = [str(binary), "--varargs-owner", str(source)]
        completed = subprocess.run(argv, capture_output=True, check=False, timeout=60)
        stdout = output / f"owner-{name}.stdout"
        stderr = output / f"owner-{name}.stderr"
        _ = stdout.write_bytes(completed.stdout)
        _ = stderr.write_bytes(completed.stderr)
        _ = (output / f"owner-{name}.json").write_text(
            json.dumps(
                {
                    "argv": argv,
                    "status": completed.returncode,
                    "binary_sha256": digest(binary),
                    "source_sha256": digest(source),
                    "stdout_sha256": digest(stdout),
                    "stderr_sha256": digest(stderr),
                },
                indent=2,
            )
            + "\n"
        )
        if completed.returncode != 0 or not completed.stdout.endswith(
            b"final alive 0\n"
        ):
            raise RuntimeError(f"Owner session failed: {name}")
    print("PASS three native owner sessions retained")


if __name__ == "__main__":
    capture(*(Path(arg).resolve() for arg in sys.argv[1:]))
