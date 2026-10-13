from __future__ import annotations

from dataclasses import dataclass

from scripts.context_ci_support import exact
from scripts.grf_control_evidence import sequence
from scripts.purchase_creation import integer, records
from scripts.world_check_support import Json, WorldCheckError, at


@dataclass(frozen=True, slots=True)
class Creation:
    vehicle: str
    ordinal: int
    orders: int
    real: int
    implicit: int


@dataclass(frozen=True, slots=True)
class Lifetime:
    live: frozenset[str]
    fresh: tuple[Creation, ...]


def occupied(snapshot: Json) -> frozenset[str]:
    values = [
        str(integer(value))
        for value in sequence(at(snapshot, ("sale", "vehicle", "pool", "occupied")))
    ]
    if len(values) != len(set(values)):
        raise WorldCheckError("Duplicate Restore vehicle identity")
    return frozenset(values)


def indices(backup: Json, orders: list[Json]) -> tuple[int, int]:
    real = integer(at(backup, ("consist", "cur_real_order_index")))
    if real >= len(orders):
        real = 0
    if any(integer(at(order, ("type",))) & 15 != 8 for order in orders):
        while integer(at(orders[real], ("type",))) & 15 == 8:
            real = (real + 1) % len(orders)
    else:
        real = 0
    implicit = integer(at(backup, ("consist", "cur_implicit_order_index")))
    if implicit >= len(orders):
        implicit = real
    return real, implicit


def allocation(
    backup: Json, before: Json, after: Json, vehicle: str
) -> tuple[int | None, int, int]:
    order_id = None
    real, implicit = 0, 0
    orders = sequence(at(backup, ("orders",)))
    if orders and integer(at(before, ("list_pool", "items"))) < 64000:
        occupied_lists = {
            integer(value) for value in sequence(at(before, ("list_pool", "occupied")))
        }
        order_id = integer(at(before, ("list_pool", "first_free")))
        while order_id in occupied_lists:
            order_id += 1
        lists = [
            entry
            for entry in sequence(at(after, ("lists",)))
            if at(entry, ("id",)) == order_id
        ]
        if (
            len(lists) != 1
            or not exact(at(lists[0], ("orders",)), orders)
            or at(lists[0], ("members",)) != [int(vehicle)]
            or at(lists[0], ("first_shared",)) != int(vehicle)
        ):
            raise WorldCheckError("Restore copied order allocation differs")
        real, implicit = indices(backup, orders)
    return order_id, real, implicit


def properties(backup: Json, before: Json, result: Json) -> None:
    for name in (
        "current_order_time",
        "current_order_time_bits",
        "lateness_counter",
        "timetable_start",
        "service_interval",
    ):
        if not exact(at(result, ("consist", name)), at(backup, ("consist", name))):
            raise WorldCheckError("Restore copied property differs")
    source_name = at(backup, ("consist", "name"))
    expected_name = (
        ""
        if any(
            at(entry, ("consist", "name")) == source_name
            for entry in sequence(at(before, ("vehicles",)))
        )
        else source_name
    )
    if (
        not exact(at(result, ("consist", "name")), expected_name)
        or integer(at(result, ("consist", "vehicle_flags"))) & 0x338
        != integer(at(backup, ("consist", "vehicle_flags"))) & 0x338
    ):
        raise WorldCheckError("Restore name/flags differ")


def created(row: Json, vehicle: str, ordinal: int) -> Creation:
    before, after = at(row, ("before", "orders")), at(row, ("after", "orders"))
    request = at(row, ("input", "request", "command"))
    user = integer(at(request, ("client_id",))) or 1
    matching = [
        backup
        for backup in sequence(at(before, ("backups",)))
        if at(backup, ("user",)) == user
        and exact(at(backup, ("tile",)), at(request, ("tile",)))
    ]
    if len(matching) > 1:
        raise WorldCheckError("Duplicate matching Restore backup")
    vehicles = [
        entry
        for entry in sequence(at(after, ("vehicles",)))
        if str(integer(at(entry, ("id",)))) == vehicle
    ]
    if len(vehicles) != 1 or at(vehicles[0], ("type",)) != 1:
        raise WorldCheckError("Restore did not create one road vehicle")
    result = vehicles[0]
    order_id: Json = None
    real, implicit = 0, 0
    if matching:
        backup = matching[0]
        if at(backup, ("clone",)) is not None or at(backup, ("group",)) != 65534:
            raise WorldCheckError("Restore lifetime exceeds owned/default-group domain")
        expected_backups = [
            entry
            for entry in sequence(at(before, ("backups",)))
            if at(entry, ("pool_slot",)) != at(backup, ("pool_slot",))
        ]
        if not exact(at(after, ("backups",)), expected_backups):
            raise WorldCheckError("Restore did not consume exactly matching backup")
        order_id, real, implicit = allocation(backup, before, after, vehicle)
        properties(backup, before, result)
    elif not exact(at(before, ("backups",)), at(after, ("backups",))):
        raise WorldCheckError("Unrelated backup changed during purchase")
    if (
        not exact(at(result, ("orders",)), order_id)
        or at(result, ("consist", "cur_real_order_index")) != real
        or at(result, ("consist", "cur_implicit_order_index")) != implicit
    ):
        raise WorldCheckError("Restore order pointer/index witness differs")
    return Creation(
        vehicle,
        ordinal,
        0 if order_id is None else integer(order_id) + 1,
        real,
        implicit,
    )


def command_lifetime(row: Json, index: int, state: Lifetime) -> Lifetime:
    live = state.live
    fresh = {entry.vehicle: entry for entry in state.fresh}
    command = at(row, ("input", "request", "command", "kind"))
    mode = at(row, ("input", "request", "mode"))
    if command not in ("build_vehicle", "sell_vehicle") or mode not in (
        "post",
        "estimate",
    ):
        raise WorldCheckError("Unsupported Restore lifetime command")
    receipt = at(row, ("result", "receipt"))
    execution = at(receipt, ("exec",))
    if execution is not None and at(execution, ("success",)) is True:
        if (
            mode != "post"
            or at(receipt, ("posted",)) is not True
            or at(receipt, ("result", "success")) is not True
        ):
            raise WorldCheckError("Uncommitted Restore lifetime")
        match command:
            case "build_vehicle":
                returned = at(receipt, ("returns", "exec"))
                vehicle = str(integer(at(returned, ("vehicle",))))
                if (
                    vehicle in live
                    or not 0 <= int(vehicle) < 1044480
                    or at(returned, ("kind",)) != "vehicle"
                    or not exact(returned, at(receipt, ("returns", "result")))
                ):
                    raise WorldCheckError("Unproven Restore creation identity")
                fresh[vehicle] = created(row, vehicle, index)
                live = live | {vehicle}
            case "sell_vehicle":
                vehicle = str(
                    integer(at(row, ("input", "request", "command", "vehicle")))
                )
                if vehicle not in live:
                    raise WorldCheckError("Sale removed a nonlive Restore incarnation")
                live = live - {vehicle}
                _ = fresh.pop(vehicle, None)
    return Lifetime(live, tuple(fresh.values()))


def lifetimes(results: Json, initial: Json, label: str) -> Lifetime:
    live = frozenset(records(initial))
    if occupied(at(results, ("initial",))) != live:
        raise WorldCheckError("Restore initial identities differ")
    fresh: dict[str, Creation] = {}
    checkpoints = {"initial": Lifetime(live, ())}
    previous = at(results, ("initial",))
    for index, row in enumerate(sequence(at(results, ("actions",)))):
        if (
            at(row, ("index",)) != index
            or not exact(at(row, ("before",)), previous)
            or occupied(at(row, ("before",))) != live
        ):
            raise WorldCheckError("Restore trace continuity differs")
        operation = at(row, ("input", "op"))
        match operation:
            case "command":
                current = command_lifetime(
                    row, index, Lifetime(live, tuple(fresh.values()))
                )
                live = current.live
                fresh = {entry.vehicle: entry for entry in current.fresh}
            case "backup" | "backup_users" | "save" | "snapshot":
                pass
            case _:
                raise WorldCheckError("Unsupported Restore lifetime action")
        previous = at(row, ("after",))
        if occupied(previous) != live:
            raise WorldCheckError("Restore complete identity set differs")
        if operation == "save":
            name = at(row, ("input", "label"))
            if not isinstance(name, str) or name in checkpoints:
                raise WorldCheckError("Duplicate Restore checkpoint")
            checkpoints[name] = Lifetime(live, tuple(fresh.values()))
    if not exact(previous, at(results, ("final",))) or label not in checkpoints:
        raise WorldCheckError("Restore final/checkpoint identity differs")
    return checkpoints[label]
