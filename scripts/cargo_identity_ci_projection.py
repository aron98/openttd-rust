from __future__ import annotations

from copy import deepcopy

from scripts.cargo_identity_ci_roster import API_PHASES
from scripts.grf_control_evidence import sequence, text
from scripts.language_ci_compare import mapping
from scripts.world_check_support import Json, WorldCheckError, at


def indices(native: Json, name: str) -> tuple[list[int], list[int]]:
    included: list[int] = []
    excluded: list[int] = []
    for index, event in enumerate(sequence(at(native, ("events",)))):
        phase = text(at(event, ("phase",)))
        admitted = (
            phase == "after-reset" if name == "loader-baseline" else phase in API_PHASES
        )
        (included if admitted else excluded).append(index)
    return included, excluded


def projection(native: Json, name: str) -> Json:
    events = sequence(at(native, ("events",)))
    included, _ = indices(native, name)
    rows: list[Json] = []
    for index in included:
        event = events[index]
        state = deepcopy(mapping(at(event, ("state",))))
        if set(state) != {
            "cargo",
            "cargo_mask",
            "standard_cargo_mask",
            "label_map",
            "climate_dependent",
            "climate_independent",
            "files",
            "engines",
        }:
            raise WorldCheckError("Cargo state schema changed")
        engines = mapping(at(state, ("engines",)))
        if set(engines) != {
            "owners",
            "mappings",
            "temporary",
            "grfid_overrides",
            "pool_capacity",
            "dynamic_engines",
            "context",
        }:
            raise WorldCheckError("Cargo engine state schema changed")
        del engines["context"]
        rows.append(
            {
                "phase": at(event, ("phase",)),
                "detail": at(event, ("detail",)),
                "state": state,
            }
        )
    return rows


def inherited_mask(native: Json, name: str) -> Json:
    phase = "before-reset" if name == "loader-baseline" else "after-finalize"
    matches = [
        row for row in sequence(at(native, ("events",))) if at(row, ("phase",)) == phase
    ]
    if len(matches) != 1:
        raise WorldCheckError("Cargo inherited context needs one predecessor")
    mask = at(matches[0], ("state", "standard_cargo_mask"))
    if type(mask) is not int or not 0 <= mask <= (1 << 64) - 1:
        raise WorldCheckError("Cargo inherited mask is not u64")
    return mask
