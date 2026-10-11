"""Mandatory bytes/modes at passive async observer producer boundaries."""

from __future__ import annotations

import hashlib
import sys
from pathlib import Path
from typing import Final

from scripts.world_check_support import Json, WorldCheckError, read_json

MANIFEST: Final = "scripts/async-trace-sources.json"
REQUIRED: Final = (
    "reference/async_trace.hpp",
    "reference/async_trace.patch",
    "reference/async_trace_input.hpp",
    "reference/async_trace_jobs.hpp",
    "reference/async_trace_state.hpp",
    "scripts/setup-async-trace-reference.sh",
    "scripts/async_trace_check.py",
    "scripts/async_trace_controls.py",
    "scripts/async_trace_host.py",
    "scripts/async_trace_jobs.py",
    "scripts/async_trace_model.py",
    "scripts/async_trace_run.py",
    "scripts/async_trace_sources.py",
    "scripts/tests/test_async_trace_sources.py",
    "scripts/world_check_support.py",
)


def mapping(value: Json) -> dict[str, Json]:
    match value:
        case dict() as entries:
            return entries
        case _:
            raise WorldCheckError("Async source inventory requires object")


def verify(root: Path) -> None:
    root = root.resolve()
    manifest = mapping(read_json(root / MANIFEST))
    hashes, modes = (mapping(manifest.get(key)) for key in ("sources", "modes"))
    if set(hashes) != set(REQUIRED) or set(modes) != set(REQUIRED):
        raise WorldCheckError("Async source inventory membership differs")
    for name in REQUIRED:
        path = root / name
        if not path.is_file() or any(
            part.is_symlink() for part in (path, *path.parents) if part != root
        ):
            raise WorldCheckError(f"Async required input missing/escaped: {name}")
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        mode = "100755" if path.stat().st_mode & 0o100 else "100644"
        if digest != hashes[name] or mode != modes[name]:
            raise WorldCheckError(f"Async source bytes/mode mismatch: {name}")


if __name__ == "__main__":
    verify(Path(sys.argv[1]).resolve())
    print(f"Async source closure verified: {len(REQUIRED)}")
