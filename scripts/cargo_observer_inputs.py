"""Mandatory passive cargo observer helper closure for complete route proofs."""

from pathlib import Path
from typing import Final

from scripts.company_rating_sources import REQUIRED as RATING_REQUIRED
from scripts.company_rating_sources import verify as verify_rating
from scripts.context_ci_support import sources
from scripts.language_ci_compare import mapping
from scripts.vehicle_year_sources import REQUIRED as YEAR_REQUIRED
from scripts.vehicle_year_sources import verify as verify_year
from scripts.world_check_support import Json, WorldCheckError, at, read_json

REQUIRED: Final = (
    "reference/cargo_runtime.hpp",
    "reference/house_cache.hpp",
    "reference/job_annotations.hpp",
    "reference/industry_cache_snapshot.hpp",
    "reference/cargo_movement_snapshot.hpp",
    "reference/cargo_replay.hpp",
    "reference/cargo_observer.patch",
    "scripts/setup-cargo-observer.sh",
    "scripts/vehicle-year-sources.json",
    *YEAR_REQUIRED,
    "scripts/company-rating-sources.json",
    *RATING_REQUIRED,
)


def verify(root: Path, layout: Json) -> None:
    """Reject missing mandatory helpers before checking every declared source byte."""
    missing = set(REQUIRED) - mapping(at(layout, ("sources",))).keys()
    if missing:
        raise WorldCheckError(
            f"Cargo observer closure omits required helpers: {sorted(missing)}"
        )
    sources(root, layout)
    verify_year(root)
    verify_rating(root)


def current() -> None:
    root = Path(__file__).resolve().parents[1]
    verify(root, read_json(root / "scripts/cargo-observer-sources.json"))
