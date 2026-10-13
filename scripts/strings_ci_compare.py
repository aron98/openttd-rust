from __future__ import annotations

import hashlib
import json
from copy import deepcopy

from scripts.grf_control_evidence import sequence, text
from scripts.language_ci_compare import compare, number, pointer_path
from scripts.world_check_support import Json, WorldCheckError, at, replace


def identities(rows: Json) -> str:
    names: list[Json] = []
    for row in sequence(rows):
        match row:
            case {"pointer": str() as pointer}:
                names.append(pointer)
            case {"operation": str() as operation}:
                names.append(operation)
            case _:
                raise WorldCheckError("Missing string control identity")
    return hashlib.sha256(json.dumps(names, separators=(",", ":")).encode()).hexdigest()


def mutation(value: Json, row: Json) -> tuple[str, Json, Json]:
    events = sequence(value)
    if at(row, ("rejected",)) is not True:
        raise WorldCheckError("Unexecuted string comparator control")
    match row:
        case {"pointer": str() as pointer, "altered": changed}:
            path = pointer_path(value, pointer)
            if len(path) < 2 or not isinstance(path[0], int):
                raise WorldCheckError("Expected string event field mutation")
            original = at(value, (path[0],))
            altered = deepcopy(original)
            replace(altered, path[1:], changed)
            return pointer, original, altered
        case {"operation": "remove-final-observation"}:
            return "remove-final-observation", value, list(events[:-1])
        case {"operation": "extra-observation"}:
            return "extra-observation", value, [*events, events[-1]]
        case {"operation": "reorder-observations", "other": other}:
            altered = list(events)
            index = number(other)
            if index == 0 or index >= len(events):
                raise WorldCheckError("Invalid string reorder index")
            altered[0], altered[index] = altered[index], altered[0]
            return "reorder-observations", value, altered
        case _:
            raise WorldCheckError("Unknown string comparator control")


def controls(value: Json, rows: Json) -> int:
    seen: set[str] = set()
    for row in sequence(rows):
        identity, original, altered = mutation(value, row)
        if identity in seen:
            raise WorldCheckError("Duplicate string comparator control")
        seen.add(identity)
        try:
            compare(original, altered)
        except WorldCheckError:
            continue
        raise WorldCheckError(f"Ineffective string mutation {text(identity)}")
    if not seen:
        raise WorldCheckError("Missing string comparator controls")
    return len(seen)
