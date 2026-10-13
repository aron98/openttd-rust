from __future__ import annotations

from scripts.grf_control_evidence import sequence, text
from scripts.language_ci_compare import compare, mapping, number, pointer_path
from scripts.world_check_support import Json, WorldCheckError, at


def expected_controls(value: Json, path: str = "") -> list[Json]:
    match value:
        case list() as values:
            rows = [
                row
                for index, item in enumerate(values)
                for row in expected_controls(item, f"{path}/{index}")
            ]
            return [
                *rows,
                {"path": path, "mutation": "extra-array-member", "rejected": True},
            ]
        case dict() as fields:
            return [
                row
                for key, item in sorted(fields.items())
                for row in expected_controls(item, f"{path}/{key}")
            ]
        case None | bool() | int() | float() | str():
            return [
                {
                    "path": path,
                    "altered": False if value is None else None,
                    "rejected": True,
                }
            ]


def controls(value: Json, rows: Json) -> int:
    required = expected_controls(value)
    compare(rows, required)
    for row in sequence(rows):
        original = at(value, pointer_path(value, text(at(row, ("path",)))))
        match row:
            case {"mutation": "extra-array-member"}:
                changed: Json = [*sequence(original), None]
            case {"altered": altered}:
                changed = altered
            case _:
                raise WorldCheckError("Unknown currency mutation")
        try:
            compare(original, changed)
        except WorldCheckError:
            continue
        raise WorldCheckError("Ineffective currency mutation")
    if not required:
        raise WorldCheckError("Missing substantive currency controls")
    return len(required)


def state(value: Json) -> Json:
    return {key: at(value, (key,)) for key in ("owners", "pending", "strings")}


def project_rows(load: Json, prefix: int) -> Json:
    rows: list[Json] = []
    for event in sequence(at(load, ("events",))):
        match text(at(event, ("phase",))):
            case "decision":
                record = mapping(at(event, ("record",)))
                index = number(at(record, ("file",)))
                if index < prefix:
                    continue
                coordinates = {
                    key: at(record, (key,)) for key in ("stage", "line", "offset")
                }
                coordinates["file"] = index - prefix
            case "stage-end":
                coordinates = {
                    "stage": at(event, ("stage",)),
                    "file": 0,
                    "line": 0,
                    "offset": 0,
                }
            case (
                "before-reset"
                | "after-reset"
                | "mapping-added"
                | "mapping-applied"
                | "before-finalize"
                | "after-finalize"
                | "finish"
            ):
                continue
            case _:
                raise WorldCheckError("Unknown original currency event")
        rows.append({"coordinates": coordinates, "state": state(event)})
    return rows


def lifecycle(load: Json) -> Json:
    return [
        {"phase": at(event, ("phase",)), "state": state(event)}
        for event in sequence(at(load, ("events",)))
        if at(event, ("phase",))
        in (
            "after-reset",
            "before-finalize",
            "mapping-applied",
            "after-finalize",
            "finish",
        )
    ]
