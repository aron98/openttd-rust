"""Command-only service scenarios over the original engine's road fleet."""

from __future__ import annotations

from dataclasses import dataclass

from scripts.world_check_support import Json, WorldCheckError, at


@dataclass(frozen=True, slots=True)
class ServiceCase:
    name: str
    intervals: tuple[int, ...]
    percent: bool = True
    custom: bool = True
    missing: bool = False


CASES = (
    ServiceCase("percent", (4, 5, 90, 91)),
    ServiceCase("calendar", (29, 30, 800, 801), percent=False),
    ServiceCase("wallclock", (0, 1, 30, 31), percent=False),
    ServiceCase("defaults-percent", (65535,), percent=False, custom=False),
    ServiceCase("defaults-days", (65535,), custom=False),
    ServiceCase("pause-gate", (42,)),
    ServiceCase("missing", (42,), missing=True),
    ServiceCase("nonprimary", (42,)),
    ServiceCase("nonowner", (0, 42)),
    ServiceCase("resume", (42, 55, 65)),
)


def plan(case: ServiceCase, vehicle: int) -> Json:
    actions: list[Json] = []
    for interval in case.intervals:
        for mode in ("estimate", "post"):
            actions.append(
                {
                    "ordinal": len(actions),
                    "op": "command",
                    "request": {
                        "company": 0,
                        "mode": mode,
                        "command": {
                            "kind": "change_service_interval",
                            "vehicle": 0xFFFFFFFF if case.missing else vehicle,
                            "interval": interval,
                            "custom": case.custom,
                            "percent": case.percent,
                        },
                    },
                }
            )
            actions.append(
                {
                    "ordinal": len(actions),
                    "op": "checkpoint",
                    "label": f"step-{len(actions)}",
                }
            )
    return {"schema_version": 1, "actions": actions}


def check_native_actions(results: Json) -> None:
    actions = at(results, ("actions",))
    match actions:
        case list():
            for index in range(len(actions)):
                before = at(results, ("actions", index, "before", "interactive_random"))
                after = at(results, ("actions", index, "after", "interactive_random"))
                if before != after:
                    raise WorldCheckError(
                        "Service unexpectedly changed native interactive RNG"
                    )
        case _:
            raise WorldCheckError("Missing native action array")
