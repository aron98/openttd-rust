from __future__ import annotations

from copy import deepcopy

from scripts.context_ci_support import exact
from scripts.gameplay_foundations import digest
from scripts.owned_restore_capacity import full_lists
from scripts.owned_restore_exports import export, verify_export
from scripts.owned_restore_native_evidence import native_receipt
from scripts.owned_restore_run import RestoreRun
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    decode_json,
    read_json,
    write_json,
)


def verify_worlds(original: Json, prepared: Json) -> None:
    expected = deepcopy(original)
    match at(expected, ("chunks", "ORDL", "records")):
        case dict() as rows:
            if any(not name.isdecimal() or not 0 <= int(name) < 64000 for name in rows):
                raise WorldCheckError("Shared capacity source IDs differ")
            for index in range(64000):
                _ = rows.setdefault(str(index), {"orders": []})
        case _:
            raise WorldCheckError("Shared capacity source rows missing")
    if not exact(expected, prepared):
        raise WorldCheckError("Shared capacity modified existing/unrelated fields")


def prepare_capacity(job: RestoreRun) -> None:
    results = job.output / "results"
    source = results / "preparation/two/original/native/initial.sav"
    destination = results / "preparation/capacity.sav"
    before = digest(source)
    _ = destination.write_bytes(full_lists(source.read_bytes()))
    for name, save in (("original", source), ("prepared", destination)):
        decoded = export(job, save, results / f"capacity-{name}-export")
        _ = (results / f"capacity-{name}.json").write_text(decoded)
    verify_worlds(
        read_json(results / "capacity-original.json"),
        read_json(results / "capacity-prepared.json"),
    )
    job.native(
        destination,
        results / "preparation/capacity-load",
        at(job.fixtures, ("capacity_load",)),
    )
    decoded = export(
        job,
        results / "preparation/capacity-load/original/native/verified.sav",
        results / "capacity-resaved-export",
    )
    _ = (results / "capacity-resaved.json").write_text(decoded)
    if (
        not exact(decode_json(decoded), read_json(results / "capacity-prepared.json"))
        or digest(source) != before
    ):
        raise WorldCheckError("Original shared capacity load/save differs")
    write_json(
        results / "capacity-preparation.json",
        {
            "source_sha256": before,
            "prepared_sha256": digest(destination),
            "records": 64000,
            "scope": "saved-input fixture only",
        },
    )
    validate_capacity(job)


def validate_capacity(job: RestoreRun) -> None:
    results = job.output / "results"
    source = results / "preparation/two/original/native/initial.sav"
    destination = results / "preparation/capacity.sav"
    if destination.read_bytes() != full_lists(source.read_bytes()):
        raise WorldCheckError("Shared capacity byte recipe differs")
    expected: Json = {
        "source_sha256": digest(source),
        "prepared_sha256": digest(destination),
        "records": 64000,
        "scope": "saved-input fixture only",
    }
    if not exact(read_json(results / "capacity-preparation.json"), expected):
        raise WorldCheckError("Shared capacity input provenance differs")
    values: dict[str, Json] = {}
    for name, save in (
        ("original", source),
        ("prepared", destination),
        ("resaved", results / "preparation/capacity-load/original/native/verified.sav"),
    ):
        values[name] = verify_export(job, save, results / f"capacity-{name}-export")
        if not exact(values[name], read_json(results / f"capacity-{name}.json")):
            raise WorldCheckError("Shared capacity exported bytes differ")
    verify_worlds(values["original"], values["prepared"])
    if not exact(values["prepared"], values["resaved"]):
        raise WorldCheckError("Shared capacity original resave changed fields")
    case = results / "preparation/capacity-load"
    native_receipt(job, case, destination)
    if not exact(
        read_json(case / "actions.json"), at(job.fixtures, ("capacity_load",))
    ):
        raise WorldCheckError("Shared capacity load descriptor differs")
    native = read_json(case / "original/native/results.json")
    if at(native, ("initial", "orders", "list_pool", "items")) != 64000 or not exact(
        at(native, ("initial",)), at(native, ("final",))
    ):
        raise WorldCheckError("Original capacity live pool/state differs")
