# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through scripts.sale_saved_state.
"""Command-only sale lifetimes; a load always starts a new ledger."""

from dataclasses import dataclass

from scripts.purchase_creation import Creation, integer, records, sequence
from scripts.world_check_support import Json, WorldCheckError, at


@dataclass(frozen=True, slots=True)
class Lifetimes:
    live: frozenset[str]
    fresh: tuple[Creation, ...]


def occupied(value: Json) -> frozenset[str]:
    match at(value, ("vehicle", "pool", "occupied")):
        case list() as ids:
            parsed = [str(integer(vehicle)) for vehicle in ids]
            if len(set(parsed)) != len(parsed):
                raise WorldCheckError("Duplicate native live vehicle identity")
            return frozenset(parsed)
        case _:
            raise WorldCheckError("Missing native sale occupancy")


def lifetimes(plan: Json, results: Json, initial: Json, label: str) -> Lifetimes:
    actions, observed = sequence(plan), sequence(results)
    if len(actions) != len(observed):
        raise WorldCheckError("Sale action count differs")
    live = set(records(initial))
    fresh: dict[str, Creation] = {}
    checkpoints = {"initial": Lifetimes(frozenset(live), ())}
    previous = -1
    for action, result in zip(actions, observed, strict=True):
        ordinal = integer(at(action, ("ordinal",)))
        operation = at(action, ("op",))
        if (
            ordinal <= previous
            or at(result, ("ordinal",)) != ordinal
            or at(result, ("op",)) != operation
        ):
            raise WorldCheckError("Sale action identity differs")
        previous = ordinal
        match operation:
            case "checkpoint":
                name = at(action, ("label",))
                match name:
                    case str() if name not in checkpoints and name != "final":
                        checkpoints[name] = Lifetimes(
                            frozenset(live), tuple(fresh.values())
                        )
                    case _:
                        raise WorldCheckError("Invalid sale checkpoint")
            case "command":
                kind = at(action, ("request", "command", "kind"))
                if kind not in ("build_vehicle", "sell_vehicle"):
                    raise WorldCheckError("Unsupported sale lifetime command")
                mode = at(action, ("request", "mode"))
                if mode not in ("estimate", "post"):
                    raise WorldCheckError("Unsupported sale mode")
                receipt = at(result, ("receipt",))
                execution = at(receipt, ("exec",))
                if execution is None or at(execution, ("success",)) is not True:
                    continue
                if (
                    mode != "post"
                    or at(receipt, ("posted",)) is not True
                    or at(receipt, ("result", "success")) is not True
                ):
                    raise WorldCheckError("Uncommitted sale lifetime execution")
                metadata = at(result, ("native_metadata",))
                if occupied(at(metadata, ("sale_before",))) != frozenset(live):
                    raise WorldCheckError(
                        "Native before occupancy contradicts sale lifetime"
                    )
                match kind:
                    case "build_vehicle":
                        returned = at(receipt, ("returns", "exec"))
                        vehicle = integer(at(returned, ("vehicle",)))
                        if (
                            at(returned, ("kind",)) != "vehicle"
                            or integer(at(receipt, ("returns", "result", "vehicle")))
                            != vehicle
                            or at(receipt, ("returns", "result", "kind")) != "vehicle"
                            or not 0 <= vehicle < 1044480
                            or str(vehicle) in live
                        ):
                            raise WorldCheckError(
                                "Build did not prove a new incarnation"
                            )
                        live.add(str(vehicle))
                        fresh[str(vehicle)] = Creation(str(vehicle), ordinal)
                    case "sell_vehicle":
                        vehicle = str(
                            integer(at(action, ("request", "command", "vehicle")))
                        )
                        if vehicle not in live:
                            raise WorldCheckError(
                                "Sale target is not a live incarnation"
                            )
                        live.remove(vehicle)
                        _ = fresh.pop(vehicle, None)
                if occupied(at(metadata, ("result", "sale"))) != frozenset(live):
                    raise WorldCheckError(
                        "Native removal or creation contradicts command target"
                    )
            case _:
                raise WorldCheckError("Unsupported sale lifetime action")
        # Each snapshot copies both sets, so later reuse cannot alter an earlier checkpoint.
    checkpoints["final"] = Lifetimes(frozenset(live), tuple(fresh.values()))
    if label not in checkpoints:
        raise WorldCheckError("Unknown sale checkpoint")
    return checkpoints[label]
