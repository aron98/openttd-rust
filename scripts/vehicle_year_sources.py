"""Mandatory bytes/modes at passive vehicle-year observer producer boundaries."""

from __future__ import annotations

import hashlib
import sys
from pathlib import Path
from typing import Final

from scripts.world_check_support import Json, WorldCheckError, read_json

MANIFEST: Final = "scripts/vehicle-year-sources.json"
REQUIRED: Final = (
    "reference/group_profit_snapshot.hpp",
    "reference/vehicle_year_trace.hpp",
    "reference/vehicle_year.patch",
    "reference/cargo_replay.hpp",
    "reference/replay_commands.hpp",
    "reference/replay_hooks.hpp",
    "reference/replay_cost.hpp",
    "scripts/setup-cargo-observer.sh",
    "scripts/setup-snapshot-reference.sh",
    "scripts/setup-vehicle-year-reference.sh",
    "scripts/vehicle_year_sources.py",
    "scripts/vehicle_year_model.py",
    "scripts/vehicle_year_run.py",
    "scripts/tests/test_vehicle_year_model.py",
    "scripts/tests/test_vehicle_year_sources.py",
    "scripts/movement_ci/value.py",
    "scripts/world_check_support.py",
)


def mapping(value: Json) -> dict[str, Json]:
    match value:
        case dict() as entries:
            return entries
        case _:
            raise WorldCheckError("Vehicle-year source inventory requires object")


def verify(root: Path) -> None:
    root = root.resolve()
    manifest = mapping(read_json(root / MANIFEST))
    hashes, modes, raw_modes = (
        mapping(manifest.get(key)) for key in ("sources", "modes", "raw_modes")
    )
    if (
        set(hashes) != set(REQUIRED)
        or set(modes) != set(REQUIRED)
        or set(raw_modes) != set(REQUIRED)
    ):
        raise WorldCheckError("Vehicle-year source inventory membership differs")
    for name in REQUIRED:
        path = root / name
        if not path.is_file() or any(
            part.is_symlink() for part in (path, *path.parents) if part != root
        ):
            raise WorldCheckError(
                f"Vehicle-year required input missing/escaped: {name}"
            )
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        mode = "100755" if path.stat().st_mode & 0o100 else "100644"
        if (
            digest != hashes[name]
            or mode != modes[name]
            or (path.stat().st_mode & 0o7777) != raw_modes[name]
        ):
            raise WorldCheckError(f"Vehicle-year source bytes/mode mismatch: {name}")


if __name__ == "__main__":
    verify(Path(sys.argv[1]).resolve())
    print(f"Vehicle-year source closure verified: {len(REQUIRED)}")
