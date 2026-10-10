"""Exact supported Rust/native cache mapping, excluding unavailable viewport state."""

from __future__ import annotations

from typing import Final

from .value import Json, array, field, integer, keys, require, same

CACHE_FIELDS: Final = frozenset(
    {
        "id",
        "road_type",
        "compatible_roadtypes",
        "first_engine",
        "vehicle_length",
        "total_length",
        "max_speed",
        "cargo_age_period",
        "visual_effect",
        "weight",
        "slope_resistance",
        "axle_resistance",
        "power",
        "max_tractive_effort",
        "max_track_speed",
        "air_drag",
        "last_speed",
        "trip_occupancy",
    }
)
EDGES: Final = ("before_save", "after_save")


def compare(native: Json, rust: Json, subject: int) -> None:
    """Compare every physical cache field and the sole valid tile bucket."""
    keys(native, set(EDGES))
    keys(rust, {"road_caches", "single_road_tile_occupancy", "capability"})
    require(
        "Rust runtime capability",
        condition=field(rust, "capability")
        == "single-road-gameplay-physical-no-viewport",
    )
    caches = array(field(rust, "road_caches"))
    require("Rust exactly one cache", condition=len(caches) == 1)
    keys(caches[0], set(CACHE_FIELDS))
    require("Rust cache subject", condition=integer(field(caches[0], "id")) == subject)
    occupancy = field(rust, "single_road_tile_occupancy")
    keys(occupancy, {"bucket", "vehicle"})
    require(
        "Rust occupancy subject",
        condition=integer(field(occupancy, "vehicle")) == subject,
    )
    for edge in EDGES:
        settled = field(native, edge)
        movement = field(field(settled, "live"), "movement")
        require(
            "native runtime unsupported",
            condition=field(movement, "runtime_road_supported") is True,
        )
        road = array(field(field(movement, "runtime_road"), "road"))
        require("native exactly one cache", condition=len(road) == 1)
        keys(road[0], set(CACHE_FIELDS))
        require(f"physical cache mismatch at {edge}", condition=same(road, caches))
        buckets = array(field(field(settled, "spatial"), "tile_hash"))
        require("native single occupied tile bucket", condition=len(buckets) == 1)
        keys(buckets[0], {"bucket", "chain"})
        require(
            "spatial bucket mismatch",
            condition=same(field(buckets[0], "bucket"), field(occupancy, "bucket")),
        )
        chain = array(field(buckets[0], "chain"))
        require("native single-vehicle chain", condition=len(chain) == 1)
        keys(chain[0], {"id", "previous_link_valid", "current_bucket_valid"})
        require(
            "native spatial subject",
            condition=integer(field(chain[0], "id")) == subject,
        )
        require(
            "native spatial link invariant",
            condition=field(chain[0], "previous_link_valid") is True
            and field(chain[0], "current_bucket_valid") is True,
        )
