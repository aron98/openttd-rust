"""Mandatory bytes/modes at passive company-rating observer producer boundaries."""

from __future__ import annotations

import hashlib
import sys
from pathlib import Path
from typing import Final

from scripts.vehicle_year_sources import verify as verify_year
from scripts.world_check_support import Json, WorldCheckError, read_json

MANIFEST: Final = "scripts/company-rating-sources.json"
REQUIRED: Final = (
    "reference/company_rating_snapshot.hpp",
    "reference/company_rating_trace.hpp",
    "reference/company_rating.patch",
    "reference/cargo_replay.hpp",
    "reference/group_profit_snapshot.hpp",
    "scripts/setup-cargo-observer.sh",
    "scripts/setup-company-rating-reference.sh",
    "scripts/setup-vehicle-year-reference.sh",
    "scripts/company_rating_sources.py",
    "scripts/company_rating_model.py",
    "scripts/company_rating_events.py",
    "scripts/company_rating_run.py",
    "scripts/tests/test_company_rating_model.py",
    "scripts/tests/test_company_rating_events.py",
    "scripts/tests/company_rating_fixture.py",
    "scripts/vehicle_year_model.py",
    "scripts/vehicle_year_sources.py",
    "scripts/movement_ci/value.py",
    "scripts/world_check_support.py",
)


def mapping(value: Json) -> dict[str, Json]:
    match value:
        case dict() as entries:
            return entries
        case _:
            raise WorldCheckError("Company-rating source inventory requires object")


def verify(root: Path) -> None:
    root = root.resolve()
    verify_year(root)
    manifest = mapping(read_json(root / MANIFEST))
    hashes, modes, raw_modes = (
        mapping(manifest.get(key)) for key in ("sources", "modes", "raw_modes")
    )
    if (
        set(hashes) != set(REQUIRED)
        or set(modes) != set(REQUIRED)
        or set(raw_modes) != set(REQUIRED)
    ):
        raise WorldCheckError("Company-rating source inventory membership differs")
    for name in REQUIRED:
        path = root / name
        if not path.is_file() or any(
            part.is_symlink() for part in (path, *path.parents) if part != root
        ):
            raise WorldCheckError(
                f"Company-rating required input missing/escaped: {name}"
            )
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        mode = "100755" if path.stat().st_mode & 0o100 else "100644"
        if (
            digest != hashes[name]
            or mode != modes[name]
            or (path.stat().st_mode & 0o7777) != raw_modes[name]
        ):
            raise WorldCheckError(f"Company-rating source bytes/mode mismatch: {name}")


if __name__ == "__main__":
    verify(Path(sys.argv[1]).resolve())
    print(f"Company-rating source closure verified: {len(REQUIRED)}")
