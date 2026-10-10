from __future__ import annotations

from scripts.context_ci_support import exact
from scripts.grf_control_evidence import sequence
from scripts.owned_restore_projection import project
from scripts.purchase_creation import common_path, integer
from scripts.sale_saved_state import compare_fields
from scripts.world_check_support import Json, WorldCheckError, at


def compare_live(native: Json, rust: Json, descriptor: Json) -> None:
    compare_fields(project(native, descriptor), rust, set())


def creations(native: Json) -> tuple[int, ...]:
    created: list[int] = []
    for row in sequence(at(native, ("actions",))):
        if at(row, ("input", "op")) != "command":
            continue
        request = at(row, ("input", "request"))
        if at(request, ("command", "kind")) != "build_vehicle":
            continue
        receipt = at(row, ("result", "receipt"))
        if at(receipt, ("exec",)) is None:
            continue
        if (
            at(receipt, ("exec", "success")) is not True
            or at(receipt, ("result", "success")) is not True
        ):
            raise WorldCheckError("Unexpected failed executed shared Build")
        if at(request, ("mode",)) != "post" or at(receipt, ("posted",)) is not True:
            raise WorldCheckError("Shared creation lacks committed Post")
        returned = at(receipt, ("returns", "result"))
        if not exact(returned, at(receipt, ("returns", "exec"))):
            raise WorldCheckError("Shared result identity mismatch")
        if at(returned, ("kind",)) != "vehicle":
            raise WorldCheckError("Shared Build did not return vehicle")
        vehicle = integer(at(returned, ("vehicle",)))
        validate_link(row, request, vehicle)
        created.append(vehicle)
    return tuple(created)


def validate_link(row: Json, request: Json, vehicle: int) -> None:
    before = at(row, ("before", "orders"))
    after = at(row, ("after", "orders"))
    user = integer(at(request, ("command", "client_id"))) or 1
    tile = integer(at(request, ("command", "tile")))
    backups = [
        entry
        for entry in sequence(at(before, ("backups",)))
        if at(entry, ("user",)) == user and at(entry, ("tile",)) == tile
    ]
    if (
        len(backups) != 1
        or at(backups[0], ("clone",)) is None
        or at(backups[0], ("group",)) != 65534
    ):
        raise WorldCheckError("Build is outside proven shared backup domain")
    clone = integer(at(backups[0], ("clone",)))
    if vehicle == clone or any(
        at(entry, ("id",)) == vehicle for entry in sequence(at(before, ("vehicles",)))
    ):
        raise WorldCheckError("Shared return is not a new incarnation")
    lists = [
        entry
        for entry in sequence(at(before, ("lists",)))
        if clone in sequence(at(entry, ("members",)))
    ]
    if len(lists) != 1:
        raise WorldCheckError("Clone lacks exactly one existing shared list")
    old = lists[0]
    new = [
        entry
        for entry in sequence(at(after, ("lists",)))
        if at(entry, ("id",)) == at(old, ("id",))
    ]
    members = list(sequence(at(old, ("members",))))
    members.insert(members.index(clone) + 1, vehicle)
    if len(new) != 1 or not exact(at(new[0], ("members",)), members):
        raise WorldCheckError("Shared insertion is not immediately after clone")
    if not exact(at(before, ("list_pool",)), at(after, ("list_pool",))):
        raise WorldCheckError("Shared Restore allocated/freed ORDL")
    if any(at(entry, ("user",)) == user for entry in sequence(at(after, ("backups",)))):
        raise WorldCheckError("Matching shared backup was not consumed")


def compare_saved(native: Json, rust: Json, created: tuple[int, ...]) -> None:
    for world in (native, rust):
        for vehicle in created:
            for field in (
                "round_trip_time",
                "depot_unbunching_last_departure",
                "depot_unbunching_next_departure",
            ):
                if integer(at(world, (*common_path(str(vehicle)), field))) != 0:
                    raise WorldCheckError(
                        "Shared creation timing must be initialized zero"
                    )
    compare_fields(native, rust, set())
