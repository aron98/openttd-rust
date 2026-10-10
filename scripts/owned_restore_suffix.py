from __future__ import annotations

import copy
from typing import Final

from scripts.context_ci_support import exact
from scripts.gameplay_foundations import digest
from scripts.order_state_evidence import command
from scripts.owned_restore_exports import export, verify_export
from scripts.owned_restore_run import RestoreRun
from scripts.purchase_creation import common_path, integer
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    read_json,
    replace,
    run,
    write_json,
)

VALUE: Final = -123456789
VEHICLE: Final = 3
FIELD: Final = (*common_path(str(VEHICLE)), "round_trip_time")


def verify_return(row: Json) -> None:
    receipt = at(row, ("result", "receipt"))
    returned = at(receipt, ("returns", "exec"))
    if (
        at(row, ("input", "op")) != "command"
        or at(row, ("input", "request", "mode")) != "post"
        or at(row, ("input", "request", "command", "kind")) != "build_vehicle"
        or at(receipt, ("posted",)) is not True
        or at(receipt, ("exec", "success")) is not True
        or at(receipt, ("result", "success")) is not True
        or at(returned, ("kind",)) != "vehicle"
        or integer(at(returned, ("vehicle",))) != VEHICLE
        or not exact(returned, at(receipt, ("returns", "result")))
    ):
        raise WorldCheckError("Suffix input lacks prior committed vehicle identity")


def verify_prepared(before: Json, after: Json) -> None:
    original = integer(at(before, FIELD))
    if integer(at(after, FIELD)) != VALUE:
        raise WorldCheckError("Suffix input lacks declared nonzero duration")
    if original == VALUE:
        raise WorldCheckError("Suffix saved-field edit is ineffective")
    expected = copy.deepcopy(before)
    replace(expected, FIELD, VALUE)
    if not exact(expected, after):
        raise WorldCheckError("Suffix preparation changed another saved field")


def edit_descriptor() -> Json:
    return {
        "schema_version": 1,
        "edits": [
            {
                "kind": "field",
                "chunk": "VEHS",
                "record": VEHICLE,
                "path": ["roadveh", 0, "common", 0, "round_trip_time"],
                "value": {"signed": VALUE},
            }
        ],
    }


def prepare_suffix(job: RestoreRun) -> None:
    results = job.output / "results"
    prior = results / "cases/owned-poison"
    directory = results / "preparation/loaded-suffix"
    directory.mkdir()
    source = prior / "original/native/after.sav"
    destination = results / "preparation/loaded-nonzero.sav"
    verify_return(at(read_json(prior / "original/native/results.json"), ("actions", 1)))
    descriptor = edit_descriptor()
    if not exact(at(job.fixtures, ("loaded_suffix_edit",)), descriptor):
        raise WorldCheckError("Suffix declared saved fixture differs")
    write_json(directory / "edit.json", descriptor)
    paths = (source, prior / "original/native/results.json", directory / "edit.json")
    before: Json = {str(path): digest(path) for path in paths}
    _ = run(
        [
            job.executable("cli"),
            "edit-world",
            str(source),
            str(directory / "edit.json"),
            str(destination),
            "--compression",
            "none",
        ],
        directory / "edit",
    )
    after: Json = {str(path): digest(path) for path in paths}
    write_json(
        directory / "bindings.json",
        {"before": before, "after": after, "output_sha256": digest(destination)},
    )
    if not exact(before, after):
        raise WorldCheckError("Suffix input or descriptor changed")
    for label, path in (("before", source), ("after", destination)):
        _ = (directory / f"{label}.json").write_text(
            export(job, path, directory / f"{label}-export")
        )
    verify_prepared(
        read_json(directory / "before.json"), read_json(directory / "after.json")
    )


def validate_suffix(job: RestoreRun) -> None:
    results = job.output / "results"
    prior = results / "cases/owned-poison"
    directory = results / "preparation/loaded-suffix"
    source = prior / "original/native/after.sav"
    destination = results / "preparation/loaded-nonzero.sav"
    verify_return(at(read_json(prior / "original/native/results.json"), ("actions", 1)))
    descriptor = edit_descriptor()
    if not exact(read_json(directory / "edit.json"), descriptor) or not exact(
        at(job.fixtures, ("loaded_suffix_edit",)), descriptor
    ):
        raise WorldCheckError("Suffix declared edit changed")
    command(
        directory / "edit",
        [
            job.executable("cli"),
            "edit-world",
            str(source),
            str(directory / "edit.json"),
            str(destination),
            "--compression",
            "none",
        ],
    )
    expected: Json = {
        str(path): digest(path)
        for path in (
            source,
            prior / "original/native/results.json",
            directory / "edit.json",
        )
    }
    if not exact(
        read_json(directory / "bindings.json"),
        {"before": expected, "after": expected, "output_sha256": digest(destination)},
    ):
        raise WorldCheckError("Suffix input binding differs")
    before = verify_export(job, source, directory / "before-export")
    after = verify_export(job, destination, directory / "after-export")
    if not exact(before, read_json(directory / "before.json")) or not exact(
        after, read_json(directory / "after.json")
    ):
        raise WorldCheckError("Suffix export bytes differ")
    verify_prepared(before, after)
    case = results / "cases/loaded-suffix"
    for name in (
        "initial-native.json",
        "rust/initial.world.json",
        "after-native.json",
        "after-rust.json",
        "original/saved-world.json",
        "rust/after.world.json",
    ):
        if not exact(after, read_json(case / name)):
            raise WorldCheckError("Declared nonzero suffix changed on load/save")
    if at(read_json(case / "after-ledger.json"), ("fresh_initialization",)) != []:
        raise WorldCheckError("Loaded suffix acquired fresh lifetime eligibility")
