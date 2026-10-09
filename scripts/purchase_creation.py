from __future__ import annotations

from dataclasses import dataclass

from scripts.world_check_support import Json, WorldCheckError, at


@dataclass(frozen=True, slots=True)
class Creation:
    vehicle: str
    ordinal: int


def integer(value: Json) -> int:
    match value:
        case int() if not isinstance(value, bool):
            return value
        case _:
            raise WorldCheckError("Expected integer purchase identity/value")


def records(world: Json) -> dict[str, Json]:
    match at(world, ("chunks", "VEHS", "records")):
        case dict() as values:
            return values
        case _:
            raise WorldCheckError("Missing vehicle records")


def sequence(value: Json) -> list[Json]:
    match at(value, ("actions",)):
        case list() as actions:
            return actions
        case _:
            raise WorldCheckError("Missing purchase action sequence")


def creations(
    plan: Json, native: Json, rust: Json, initial: Json, label: str
) -> tuple[Creation, ...]:
    actions, expected, actual = sequence(plan), sequence(native), sequence(rust)
    if len(actions) != len(expected) or len(actions) != len(actual):
        raise WorldCheckError("Purchase action count differs")
    initial_ids = records(initial).keys()
    created: dict[str, Creation] = {}
    checkpoints: dict[str, tuple[Creation, ...]] = {"initial": ()}
    previous = -1
    for action, result in zip(actions, expected, strict=True):
        ordinal = integer(at(action, ("ordinal",)))
        operation = at(action, ("op",))
        if (
            ordinal <= previous
            or at(result, ("ordinal",)) != ordinal
            or at(result, ("op",)) != operation
        ):
            raise WorldCheckError("Purchase action identity differs")
        previous = ordinal
        match operation:
            case "checkpoint":
                name = at(action, ("label",))
                match name:
                    case str() if name not in checkpoints and name != "final":
                        checkpoints[name] = tuple(created.values())
                    case _:
                        raise WorldCheckError("Invalid purchase checkpoint")
            case "command":
                if at(action, ("request", "command", "kind")) != "build_vehicle":
                    raise WorldCheckError("Unsupported purchase lifetime command")
                mode = at(action, ("request", "mode"))
                if mode not in ("estimate", "post"):
                    raise WorldCheckError("Unsupported purchase command mode")
                receipt = at(result, ("receipt",))
                execution = at(receipt, ("exec",))
                if execution is None or at(execution, ("success",)) is not True:
                    continue
                if mode != "post" or at(receipt, ("result", "success")) is not True:
                    raise WorldCheckError("Uncommitted purchase execution")
                returned = at(receipt, ("returns", "exec"))
                vehicle = integer(at(returned, ("vehicle",)))
                if (
                    at(returned, ("kind",)) != "vehicle"
                    or at(receipt, ("returns", "result", "vehicle")) != vehicle
                    or not 0 <= vehicle < 1044480
                    or str(vehicle) in initial_ids
                    or str(vehicle) in created
                ):
                    raise WorldCheckError(
                        "Unproven or reused purchase vehicle identity"
                    )
                created[str(vehicle)] = Creation(str(vehicle), ordinal)
            case _:
                raise WorldCheckError("Unsupported purchase lifetime action")
    checkpoints["final"] = tuple(created.values())
    if label not in checkpoints:
        raise WorldCheckError("Unknown purchase checkpoint")
    return checkpoints[label]


def common_path(vehicle: str) -> tuple[str | int, ...]:
    return ("chunks", "VEHS", "records", vehicle, "roadveh", 0, "common", 0)


def fresh_duration(world: Json, creation: Creation) -> int:
    path = common_path(creation.vehicle)
    road = at(world, path[:-2])
    match road:
        case {"state": 254, "common": [dict()]}:
            pass
        case _:
            raise WorldCheckError("Fresh purchase is not a single depot road record")
    common = at(world, path)
    for field in (
        "orders",
        "next",
        "next_shared",
        "cur_speed",
        "tick_counter",
        "current_order.type",
        "current_order.flags",
        "cur_real_order_index",
        "cur_implicit_order_index",
        "depot_unbunching_last_departure",
        "depot_unbunching_next_departure",
    ):
        if integer(at(common, (field,))) != 0:
            raise WorldCheckError(
                f"Fresh purchase domain changed: {creation.vehicle}.{field}"
            )
    if (
        integer(at(common, ("subtype",))) != 1
        or integer(at(common, ("vehstatus",))) != 11
    ):
        raise WorldCheckError("Fresh purchase is not the stopped depot vehicle")
    value = integer(at(common, ("round_trip_time",)))
    if not -(2**31) <= value < 2**31:
        raise WorldCheckError("Original fresh duration is not a signed int32")
    return value
