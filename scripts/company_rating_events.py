"""Require complete native company-month callback sequences without rewriting them."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from scripts.company_rating_model import Snapshot, snapshot
from scripts.movement_ci.value import DECODE, Json, field, keys, require, string
from scripts.vehicle_year_model import number


@dataclass(frozen=True, slots=True)
class Event:
    sequence: int
    phase: str
    owner: int
    snapshot: Snapshot


def event(raw: Json) -> Event:
    keys(raw, {"sequence", "phase", "owner", "snapshot"})
    return Event(
        number(field(raw, "sequence"), 0, 2**64 - 1),
        string(field(raw, "phase")),
        number(field(raw, "owner"), 0, 255),
        snapshot(field(raw, "snapshot")),
    )


def ordered(events: tuple[Event, ...]) -> None:
    cursor = 0
    while cursor < len(events):
        first = events[cursor]
        expected = [
            (name, 255)
            for name in (
                "month_enter",
                "statistics_enter",
                "bankruptcy_complete",
                "maintenance_complete",
            )
        ]
        start = cursor + len(expected)
        if first.snapshot.economy_month % 3 == 0 and start <= len(events):
            owners = events[start - 1].snapshot.companies
            expected.extend(
                (name, company.owner)
                for company in owners
                for name in (
                    "history_before",
                    "history_after",
                    "rating_complete",
                )
            )
        expected.extend(
            (name, 255) for name in ("interest_before", "interest_after", "month_leave")
        )
        require(
            "incomplete company month", condition=cursor + len(expected) <= len(events)
        )
        clock = field(first.snapshot.runtime.raw, "clock")
        for phase, owner in expected:
            observed = events[cursor]
            require(
                "company phase order differs",
                condition=(observed.phase, observed.owner) == (phase, owner),
            )
            require(
                "company month clock changed",
                condition=field(observed.snapshot.runtime.raw, "clock") == clock,
            )
            require("company sequence differs", condition=observed.sequence == cursor)
            cursor += 1


def read_events(path: Path) -> tuple[Event, ...]:
    with path.open() as stream:
        events = tuple(event(DECODE(line)) for line in stream)
    ordered(events)
    return events
