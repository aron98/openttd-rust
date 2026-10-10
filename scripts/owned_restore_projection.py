from __future__ import annotations

from copy import deepcopy

from scripts.context_ci_support import exact
from scripts.depot_removal_pair import projection
from scripts.grf_control_evidence import sequence
from scripts.order_state_pair import object_at
from scripts.world_check_support import Json, WorldCheckError, at


def project(native: Json, descriptor: Json) -> Json:
    """Validate original purchase cache witnesses before standard host projection."""
    copied = deepcopy(native)
    for row in sequence(at(copied, ("actions",))):
        if at(row, ("input", "op")) != "command":
            continue
        if at(row, ("input", "request", "command", "kind")) != "build_vehicle":
            continue
        metadata = object_at(at(row, ("result", "native_metadata")))
        if not exact(
            metadata.pop("purchase_before", None),
            at(row, ("before", "sale", "vehicle")),
        ):
            raise WorldCheckError("Restore purchase-before cache witness differs")
        for phase, value in metadata.items():
            values = object_at(value)
            expected = at(
                row, ("before" if phase == "test" else "after", "sale", "vehicle")
            )
            if not exact(values.pop("live", None), expected):
                raise WorldCheckError("Restore phase cache witness differs")
    return projection(copied, descriptor)
