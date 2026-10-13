from __future__ import annotations

import json
from dataclasses import dataclass
from enum import StrEnum
from typing import Final, NewType, TypeAlias

Json: TypeAlias = dict[str, "Json"] | list["Json"] | str | int | float | bool | None
EngineId = NewType("EngineId", int)
VehicleId = NewType("VehicleId", int)


class ProtocolError(ValueError):
    pass


class VehicleKind(StrEnum):
    BUS = "bus"
    TRUCK = "truck"


@dataclass(frozen=True, slots=True)
class Case:
    name: str
    kind: VehicleKind
    engine: EngineId
    reservations: int


CASES: Final = (
    Case("bus-first", VehicleKind.BUS, EngineId(116), 0),
    Case("bus-first-stride", VehicleKind.BUS, EngineId(116), 1),
    Case("truck-first", VehicleKind.TRUCK, EngineId(123), 0),
    Case("truck-second-stride", VehicleKind.TRUCK, EngineId(123), 75),
)
SCHEMA: Final = 2
MAX_DISCOVERY: Final = 128
MAX_PREPARE: Final = 512
MAX_QUALIFICATION: Final = 256


def bounded(value: Json, maximum: int) -> int:
    if (
        isinstance(value, bool)
        or not isinstance(value, int)
        or not 0 <= value <= maximum
    ):
        raise ProtocolError("bounded integer required")
    return value


def exact(left: Json, right: Json) -> bool:
    try:
        return json.dumps(left, sort_keys=True, allow_nan=False) == json.dumps(
            right, sort_keys=True, allow_nan=False
        )
    except ValueError as error:
        raise ProtocolError("non-finite protocol number") from error


def post(command: dict[str, Json]) -> Json:
    return {"company": 0, "mode": "post", "command": command}


def prepare(case: Case) -> dict[str, Json]:
    _ = bounded(case.reservations, 75)
    if case not in CASES:
        raise ProtocolError("case is outside the complete portfolio")
    commands: list[Json] = [
        post(
            {
                "kind": "build_road",
                "tile": 640 + x,
                "pieces": 10,
                "road_type": 0,
                "toggle_disallowed": 0,
                "town_id": 65535,
            }
        )
        for x in range(8, 33)
    ]
    commands.extend(
        [
            post(
                {
                    "kind": "build_road_depot",
                    "tile": 673,
                    "road_type": 0,
                    "direction": 0,
                }
            ),
            post(
                {
                    "kind": "build_vehicle",
                    "tile": 673,
                    "engine": int(case.engine),
                    "use_free_vehicles": False,
                    "cargo": 255,
                    "client_id": 0,
                }
            ),
        ]
    )
    return {
        "schema_version": SCHEMA,
        "kind": "road_movement",
        "mode": "prepare",
        "trace": True,
        "max_calls": MAX_PREPARE,
        "max_bytes": 805306368,
        "max_seconds": 60,
        "max_events": 200000,
        "reservation_count": case.reservations,
        "prepare_settings": [
            post(
                {
                    "kind": "movement_prepare_original_acceleration",
                    "name": "vehicle.roadveh_acceleration_model",
                    "value": 0,
                }
            )
        ],
        "commands": commands,
        "witness_tiles": [640 + x for x in range(10, 31)],
    }


def loaded(
    mode: str, subject: VehicleId, calls: int, *, trace: bool
) -> dict[str, Json]:
    if mode not in {"replay", "discover"}:
        raise ProtocolError("loaded mode required")
    checked_subject = bounded(int(subject), (1 << 32) - 2)
    checked_calls = bounded(calls, MAX_QUALIFICATION)
    if mode == "discover" and calls != 0:
        raise ProtocolError("discovery horizon is measured, not supplied")
    result: dict[str, Json] = {
        "schema_version": SCHEMA,
        "kind": "road_movement",
        "mode": mode,
        "subject": checked_subject,
        "trace": trace,
        "max_calls": MAX_DISCOVERY if mode == "discover" else MAX_QUALIFICATION,
        "max_bytes": 402653184,
        "max_seconds": 60,
        "max_events": 200000,
    }
    if mode == "replay":
        result["calls"] = checked_calls
    return result


def validate_request(request: dict[str, Json]) -> None:
    common = {
        "schema_version",
        "kind",
        "mode",
        "trace",
        "max_calls",
        "max_bytes",
        "max_seconds",
        "max_events",
    }
    mode = request.get("mode")
    if mode == "prepare":
        if set(request) != common | {
            "reservation_count",
            "prepare_settings",
            "commands",
            "witness_tiles",
        }:
            raise ProtocolError("preparation key set")
        if not any(exact(request, prepare(case)) for case in CASES):
            raise ProtocolError("preparation must match an exact command recipe")
        return
    if mode not in {"replay", "discover"}:
        raise ProtocolError("unknown mode")
    expected = common | {"subject"}
    if mode == "replay":
        expected.add("calls")
    if set(request) != expected:
        raise ProtocolError("loaded key set excludes preparation/settings/commands")
    subject = VehicleId(bounded(request["subject"], (1 << 32) - 2))
    calls = bounded(request.get("calls", 0), MAX_QUALIFICATION)
    trace = request.get("trace")
    if not isinstance(trace, bool) or not exact(
        request, loaded(mode, subject, calls, trace=trace)
    ):
        raise ProtocolError("loaded request differs from bounded protocol")


def qualification_horizon(crossing: Json) -> int:
    value = bounded(crossing, MAX_DISCOVERY)
    if value == 0:
        raise ProtocolError("positive original crossing required")
    return max(74, 2 * value)


def labels(calls: int) -> list[str]:
    checked_calls = bounded(calls, MAX_QUALIFICATION)
    return [
        "initial",
        *[f"tick_{call}" for call in range(1, checked_calls + 1)],
        "final",
    ]


def require_labels(actual: list[str], calls: int) -> None:
    if actual != labels(calls):
        raise ProtocolError("missing/extra/reordered/subset checkpoint horizon")


def require_cases(actual: list[str]) -> None:
    if actual != [case.name for case in CASES]:
        raise ProtocolError("complete ordered four-case portfolio required")


def cli_plan(calls: int) -> dict[str, Json]:
    checked_calls = bounded(calls, MAX_QUALIFICATION)
    actions: list[Json] = []
    for call in range(1, checked_calls + 1):
        actions.extend(
            [
                {"op": "tick", "ordinal": 2 * (call - 1), "count": 1},
                {
                    "op": "checkpoint",
                    "ordinal": 2 * (call - 1) + 1,
                    "label": f"tick_{call}",
                },
            ]
        )
    return {"schema_version": 1, "actions": actions}
