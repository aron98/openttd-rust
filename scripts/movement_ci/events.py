"""Native phase-order and entry/RNG receipts, checked without a second scheduler."""

from __future__ import annotations

from pathlib import Path

from .value import DECODE, Json, field, integer, require, same, string


def validate(directory: Path, result: Json) -> None:
    sequence = 0
    stack: list[tuple[str, int, int]] = []
    calls: list[int] = []
    with (directory / "events.jsonl").open() as stream:
        for line in stream:
            event = DECODE(line)
            require(
                "event sequence gap",
                condition=integer(field(event, "sequence")) == sequence,
            )
            sequence += 1
            key = (
                string(field(event, "phase")),
                integer(field(event, "vehicle")),
                integer(field(event, "tile")),
            )
            if field(event, "edge") == "enter":
                stack.append(key)
                if field(event, "phase") == "state_loop":
                    calls.append(integer(field(event, "call")))
            if field(event, "edge") == "leave":
                require(
                    "native phase stack order",
                    condition=bool(stack) and stack.pop() == key,
                )
    trace = field(result, "trace") is True
    expected = list(range(1, integer(field(result, "calls")) + 1)) if trace else []
    require(
        "incomplete event stream",
        condition=not stack and sequence == integer(field(result, "events")),
    )
    require("trace activation mismatch", condition=(sequence > 0) == trace)
    require("complete native tick roster", condition=calls == expected)


def crossing(directory: Path, result: Json) -> tuple[int, int]:
    entries = 0
    consumed = 0
    before: Json = None
    with (directory / "events.jsonl").open() as stream:
        for line in stream:
            event = DECODE(line)
            if field(event, "call") != field(result, "calls"):
                continue
            if (
                field(event, "phase") == "vehicle_enter_tile"
                and field(event, "edge") == "result"
                and field(event, "vehicle") == field(result, "subject")
            ):
                entries += 1
            if (
                field(event, "phase") == "pick_random_bit"
                and field(event, "edge") == "arguments"
            ):
                require("nested random arguments", condition=before is None)
                before = event
            if (
                field(event, "phase") == "pick_random_bit"
                and field(event, "edge") == "random_result"
            ):
                require("random result missing arguments", condition=before is not None)
                if field(before, "b") == 1:
                    require(
                        "one-choice original RNG did not advance",
                        condition=not same(
                            field(field(before, "live"), "random"),
                            field(field(event, "live"), "random"),
                        ),
                    )
                    consumed += 1
                before = None
    require(
        "crossing entry/RNG evidence missing",
        condition=entries > 0 and consumed > 0 and before is None,
    )
    return entries, consumed
