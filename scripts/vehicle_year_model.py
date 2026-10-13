"""Require actual native group-profit coordinates without projection or defaults."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Final

from scripts.movement_ci.value import (
    DECODE,
    EvidenceError,
    Json,
    array,
    field,
    integer,
    keys,
    require,
    string,
)


@dataclass(frozen=True, slots=True)
class Statistics:
    profit_last_year: int
    profit_last_year_min_age: int
    num_vehicle_min_age: int
    num_vehicle: int
    engines: tuple[tuple[int, int], ...]
    autoreplace_defined: bool
    autoreplace_finished: bool


@dataclass(frozen=True, slots=True)
class Group:
    owner: int
    vehicle_type: int
    group: int
    parent: int
    statistics: Statistics


@dataclass(frozen=True, slots=True)
class Snapshot:
    groups: tuple[Group, ...]
    raw: Json


@dataclass(frozen=True, slots=True)
class Event:
    sequence: int
    phase: str
    vehicle: int
    extra: Json
    snapshot: Snapshot


MAX_NEWS_AMOUNT: Final = 1 << 10


def number(value: Json, lower: int, upper: int) -> int:
    result = integer(value)
    require("native scalar out of range", condition=lower <= result <= upper)
    return result


def boolean(value: Json) -> bool:
    if not isinstance(value, bool):
        raise EvidenceError("native boolean required")
    return value


def statistics(raw: Json) -> Statistics:
    keys(
        raw,
        {
            "profit_last_year",
            "profit_last_year_min_age",
            "num_vehicle_min_age",
            "num_vehicle",
            "num_engines",
            "autoreplace_defined",
            "autoreplace_finished",
        },
    )
    engines: list[tuple[int, int]] = []
    for entry in array(field(raw, "num_engines")):
        pair = array(entry)
        require("engine pair required", condition=len(pair) == 2)
        engines.append((number(pair[0], 0, 65535), number(pair[1], 0, 65535)))
    require(
        "duplicate engine identity",
        condition=len({engine for engine, _ in engines}) == len(engines),
    )
    return Statistics(
        number(
            field(raw, "profit_last_year"), -9223372036854775808, 9223372036854775807
        ),
        number(
            field(raw, "profit_last_year_min_age"),
            -9223372036854775808,
            9223372036854775807,
        ),
        number(field(raw, "num_vehicle_min_age"), 0, 65535),
        number(field(raw, "num_vehicle"), 0, 65535),
        tuple(engines),
        boolean(field(raw, "autoreplace_defined")),
        boolean(field(raw, "autoreplace_finished")),
    )


def snapshot(raw: Json) -> Snapshot:
    keys(
        raw,
        {
            "schema_version",
            "companies",
            "groups",
            "vehicles",
            "news_queue",
            "context",
            "clock",
        },
    )
    require(
        "unsupported group schema", condition=integer(field(raw, "schema_version")) == 1
    )
    groups: list[Group] = []
    owners: set[int] = set()
    for company in array(field(raw, "companies")):
        keys(company, {"id", "is_ai", "all", "default", "yearly_expenses"})
        owner = number(field(company, "id"), 0, 14)
        require("duplicate company", condition=owner not in owners)
        owners.add(owner)
        _ = boolean(field(company, "is_ai"))
        for name, group_id in (("all", 65533), ("default", 65534)):
            entries = array(field(company, name))
            require(
                "four native company vehicle types required",
                condition=len(entries) == 4,
            )
            groups.extend(
                Group(owner, kind, group_id, 65535, statistics(entry))
                for kind, entry in enumerate(entries)
            )
        years = array(field(company, "yearly_expenses"))
        require("three expense years required", condition=len(years) == 3)
        for year in years:
            for expense in array(year):
                _ = number(expense, -9223372036854775808, 9223372036854775807)
    named: set[int] = set()
    for group in array(field(raw, "groups")):
        keys(group, {"id", "owner", "type", "parent", "statistics"})
        group_id = number(field(group, "id"), 0, 65532)
        owner = number(field(group, "owner"), 0, 14)
        require(
            "invalid or duplicate group ownership",
            condition=owner in owners and group_id not in named,
        )
        named.add(group_id)
        groups.append(
            Group(
                owner,
                number(field(group, "type"), 0, 3),
                group_id,
                number(field(group, "parent"), 0, 65535),
                statistics(field(group, "statistics")),
            )
        )
    validate_context(raw)
    _ = number(field(raw, "news_queue"), 0, MAX_NEWS_AMOUNT)
    return Snapshot(tuple(groups), raw)


def validate_context(raw: Json) -> None:
    vehicles: set[int] = set()
    for vehicle in array(field(raw, "vehicles")):
        keys(
            vehicle,
            {
                "id",
                "owner",
                "type",
                "group",
                "primary",
                "economy_age",
                "profit_this_year",
                "profit_last_year",
            },
        )
        vehicle_id = number(field(vehicle, "id"), 0, 4294967295)
        require("duplicate vehicle", condition=vehicle_id not in vehicles)
        vehicles.add(vehicle_id)
        _ = boolean(field(vehicle, "primary"))
        for key in ("owner", "type", "group", "economy_age"):
            _ = number(field(vehicle, key), 0, 4294967295)
        for key in ("profit_this_year", "profit_last_year"):
            _ = number(field(vehicle, key), -9223372036854775808, 9223372036854775807)
    context = field(raw, "context")
    keys(
        context,
        {
            "current_company",
            "local_company",
            "networking",
            "network_server",
            "vehicle_income_warn",
            "old_vehicle_warn",
        },
    )
    for key in ("current_company", "local_company"):
        _ = number(field(context, key), 0, 255)
    for key in (
        "networking",
        "network_server",
        "vehicle_income_warn",
        "old_vehicle_warn",
    ):
        _ = boolean(field(context, key))
    clock = field(raw, "clock")
    keys(
        clock,
        {
            "tick",
            "economy_date",
            "economy_fract",
            "economy_year",
            "calendar_date",
            "calendar_fract",
        },
    )
    for key in (
        "tick",
        "economy_date",
        "economy_fract",
        "economy_year",
        "calendar_date",
        "calendar_fract",
    ):
        _ = integer(field(clock, key))


def read_snapshot(path: Path) -> Snapshot:
    return snapshot(DECODE(path.read_text()))


def probe(phase: str, raw: Json) -> Json:
    """Validate the probe context of one year-trace event against the native branch."""
    if phase == "unprofitable_news":
        keys(raw, {"company", "string", "profit"})
        _ = number(field(raw, "company"), 0, 14)
        _ = number(field(raw, "string"), 0, 2**32 - 1)
        _ = number(field(raw, "profit"), -(2**63), 2**63 - 1)
        return raw
    if phase == "unprofitable_ai":
        keys(raw, {"company", "is_ai"})
        _ = number(field(raw, "company"), 0, 14)
        _ = boolean(field(raw, "is_ai"))
        return raw
    require(
        "unexpected probe context on a passive year phase",
        condition=raw is None,
    )
    return raw


def read_events(path: Path) -> tuple[Event, ...]:
    events: list[Event] = []
    with path.open() as stream:
        for line in stream:
            raw = DECODE(line)
            keys(raw, {"sequence", "phase", "vehicle", "extra", "snapshot"})
            sequence = integer(field(raw, "sequence"))
            require(
                "noncontiguous year event sequence", condition=sequence == len(events)
            )
            phase = string(field(raw, "phase"))
            require(
                "unknown year phase",
                condition=phase
                in {
                    "company_year_enter",
                    "company_year_leave",
                    "town_year_enter",
                    "town_year_leave",
                    "vehicle_year_enter",
                    "vehicle_year_leave",
                    "group_profits_before",
                    "group_profits_after",
                    "age731_before",
                    "age731_after",
                    "unprofitable_news",
                    "unprofitable_ai",
                },
            )
            extra = probe(phase, field(raw, "extra"))
            events.append(
                Event(
                    sequence,
                    phase,
                    number(field(raw, "vehicle"), 0, 4294967295),
                    extra,
                    snapshot(field(raw, "snapshot")),
                )
            )
    return tuple(events)
