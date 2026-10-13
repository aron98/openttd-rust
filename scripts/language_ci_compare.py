from __future__ import annotations

from copy import deepcopy

from scripts.context_ci_support import exact
from scripts.grf_control_evidence import sequence, text
from scripts.world_check_support import Json, WorldCheckError, at, replace


def compare(native: Json, rust: Json) -> None:
    if not exact(native, rust):
        raise WorldCheckError("Original language observables differ")


def mapping(value: Json) -> dict[str, Json]:
    match value:
        case dict() as fields:
            return fields
        case _:
            raise WorldCheckError("Expected language observation object")


def number(value: Json) -> int:
    match value:
        case bool():
            raise WorldCheckError("Boolean language index")
        case int() as result if result >= 0:
            return result
        case _:
            raise WorldCheckError("Invalid language index")


def pointer_path(value: Json, pointer: str) -> tuple[str | int, ...]:
    if pointer == "":
        return ()
    if not pointer.startswith("/"):
        raise WorldCheckError("Invalid language mutation pointer")
    path: list[str | int] = []
    for token in pointer[1:].split("/"):
        key = token.replace("~1", "/").replace("~0", "~")
        match value:
            case list():
                if not key.isdecimal():
                    raise WorldCheckError("Invalid language array pointer")
                component: str | int = int(key)
            case dict():
                component = key
            case _:
                raise WorldCheckError("Mutation descends through scalar")
        path.append(component)
        value = at(value, (component,))
    return tuple(path)


def mutation(value: Json, row: Json) -> tuple[str, Json]:
    if at(row, ("rejected",)) is not True:
        raise WorldCheckError("Language comparator did not reject")
    match row:
        case {"pointer": str() as pointer, "before": before, "after": after}:
            path = pointer_path(value, pointer)
            compare(at(value, path), before)
            altered = deepcopy(value)
            if not path:
                altered = after
            else:
                replace(altered, path, after)
            return pointer, altered
        case {"operation": "append-null"}:
            altered = list(sequence(value))
            altered.append(None)
            return "append-null", altered
        case _:
            raise WorldCheckError("Unknown language comparator control")


def mutations(value: Json, rows: Json) -> int:
    seen: set[str] = set()
    for row in sequence(rows):
        identity, altered = mutation(value, row)
        if identity in seen:
            raise WorldCheckError("Duplicate language mutation identity")
        seen.add(identity)
        try:
            compare(value, altered)
        except WorldCheckError:
            continue
        raise WorldCheckError("Language mutation is ineffective")
    if not seen:
        raise WorldCheckError("Missing language comparator controls")
    return len(seen)


def strip_baseline(value: Json, prefix: list[int]) -> Json:
    result = deepcopy(mapping(value))
    if "files" in result:
        files = sequence(result["files"])
        if len(files) < len(prefix):
            raise WorldCheckError("Missing baseline state")
        for row, identity in zip(files[: len(prefix)], prefix, strict=True):
            if number(at(row, ("config_grfid",))) != identity or at(
                row, ("file_grfid",)
            ) not in (None, identity):
                raise WorldCheckError("Unsupported baseline registry identity")
        result["files"] = files[len(prefix) :]
    if "overrides" in result:
        result["overrides"] = [
            row
            for row in sequence(result["overrides"])
            if number(at(row, ("config_grfid",))) not in prefix
        ]
    return result


def control_projection(raw: Json) -> Json:
    prefix = [
        number(at(row, ("config_grfid",))) for row in sequence(at(raw, ("files",)))[:2]
    ]
    if len(prefix) != 2 or len(set(prefix)) != 2:
        raise WorldCheckError("Missing distinct original baseline identities")
    events: list[Json] = []
    for row in sequence(at(raw, ("events",))):
        event = deepcopy(mapping(row))
        if text(at(event, ("kind",))) == "record":
            index = number(at(event, ("file",)))
            if index < 2:
                continue
            event["file"] = index - 2
        events.append(strip_baseline(event, prefix))
    result = mapping(
        strip_baseline(
            {"files": at(raw, ("files",)), "overrides": at(raw, ("overrides",))}, prefix
        )
    )
    result["events"] = events
    return result
