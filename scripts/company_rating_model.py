"""Parse all native raw rating slots and their unmodified company context."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Final

from scripts.movement_ci.value import DECODE, Json, array, field, keys, require
from scripts.vehicle_year_model import Snapshot as GroupSnapshot
from scripts.vehicle_year_model import number
from scripts.vehicle_year_model import snapshot as groups

MIN: Final = -(2**63)
MAX: Final = 2**63 - 1
INFO: Final = (
    (120, 100),
    (80, 100),
    (10000, 100),
    (50000, 50),
    (100000, 100),
    (40000, 400),
    (8, 50),
    (10000000, 50),
    (250000, 50),
    (0, 0),
)


@dataclass(frozen=True, slots=True)
class Economy:
    income: int
    expenses: int
    delivered: tuple[int, ...]
    performance: int
    value: int


@dataclass(frozen=True, slots=True)
class Company:
    owner: int
    valid_history: int
    block_preview: int
    money: int
    loan: int
    hq_tile: int
    current: Economy
    history: tuple[Economy, ...]


@dataclass(frozen=True, slots=True)
class Snapshot:
    economy_month: int
    scores: tuple[tuple[int, ...], ...]
    companies: tuple[Company, ...]
    runtime: GroupSnapshot


def score_rows(raw: Json) -> tuple[tuple[int, ...], ...]:
    rows = array(raw)
    require("all fifteen native score slots required", condition=len(rows) == 15)
    result: list[tuple[int, ...]] = []
    for owner, row in enumerate(rows):
        keys(row, {"owner", "parts"})
        require(
            "score owner order differs",
            condition=number(field(row, "owner"), 0, 14) == owner,
        )
        parts = tuple(number(value, MIN, MAX) for value in array(field(row, "parts")))
        require("all ten raw score parts required", condition=len(parts) == 10)
        result.append(parts)
    return tuple(result)


def economy(raw: Json) -> Economy:
    keys(
        raw,
        {
            "income",
            "expenses",
            "delivered_cargo",
            "performance_history",
            "company_value",
        },
    )
    delivered = tuple(
        number(value, 0, 2**32 - 1) for value in array(field(raw, "delivered_cargo"))
    )
    require("all native cargo slots required", condition=len(delivered) == 64)
    return Economy(
        number(field(raw, "income"), MIN, MAX),
        number(field(raw, "expenses"), MIN, MAX),
        delivered,
        number(field(raw, "performance_history"), -(2**31), 2**31 - 1),
        number(field(raw, "company_value"), MIN, MAX),
    )


def company(raw: Json) -> Company:
    keys(
        raw,
        {
            "id",
            "valid_history",
            "block_preview",
            "money",
            "loan",
            "hq_tile",
            "current",
            "history",
        },
    )
    history = tuple(economy(row) for row in array(field(raw, "history")))
    require("all twenty-four history slots required", condition=len(history) == 24)
    return Company(
        number(field(raw, "id"), 0, 14),
        number(field(raw, "valid_history"), 0, 24),
        number(field(raw, "block_preview"), 0, 255),
        number(field(raw, "money"), MIN, MAX),
        number(field(raw, "loan"), MIN, MAX),
        number(field(raw, "hq_tile"), 0, 2**32 - 1),
        economy(field(raw, "current")),
        history,
    )


def snapshot(raw: Json) -> Snapshot:
    keys(
        raw,
        {
            "schema_version",
            "economy_month",
            "scores",
            "score_info",
            "companies",
            "runtime",
        },
    )
    _ = number(field(raw, "schema_version"), 1, 1)
    info = array(field(raw, "score_info"))
    require("all score definitions required", condition=len(info) == 10)
    for index, row in enumerate(info):
        keys(row, {"id", "needed", "weight"})
        _ = number(field(row, "id"), index, index)
        actual = (
            number(field(row, "needed"), 0, MAX),
            number(field(row, "weight"), 0, MAX),
        )
        require("pinned score definition differs", condition=actual == INFO[index])
    companies = tuple(company(row) for row in array(field(raw, "companies")))
    owners = tuple(row.owner for row in companies)
    require(
        "company owner ordering differs", condition=owners == tuple(sorted(set(owners)))
    )
    runtime_raw = field(raw, "runtime")
    runtime = groups(runtime_raw)
    observed = tuple(
        number(field(row, "id"), 0, 14)
        for row in array(field(runtime_raw, "companies"))
    )
    require("company membership differs", condition=owners == observed)
    for vehicle in array(field(runtime_raw, "vehicles")):
        kind = number(field(vehicle, "type"), 0, 255)
        if kind < 4:
            require(
                "buildable vehicle owner absent",
                condition=number(field(vehicle, "owner"), 0, 14) in owners,
            )
    return Snapshot(
        number(field(raw, "economy_month"), 0, 11),
        score_rows(field(raw, "scores")),
        companies,
        runtime,
    )


def read_snapshot(path: Path) -> Snapshot:
    return snapshot(DECODE(path.read_text()))
