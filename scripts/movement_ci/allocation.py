"""Recipe-specific original allocation receipts, not a general pool implementation."""

from __future__ import annotations

from dataclasses import dataclass

from .protocol import Case
from .value import Json, array, field, integer, keys, require


@dataclass(frozen=True, slots=True)
class Allocation:
    occupied: tuple[int, ...]
    first_free: int
    first_unused: int
    slots: int
    next_unit: int

    def check(self, value: Json) -> None:
        keys(value, {"pool", "road", "units"})
        pool = field(value, "pool")
        keys(pool, {"occupied", "items", "first_free", "first_unused", "slots"})
        observed = tuple(integer(item) for item in array(field(pool, "occupied")))
        require("allocation occupied sequence", condition=observed == self.occupied)
        require(
            "allocation item count",
            condition=integer(field(pool, "items")) == len(self.occupied),
        )
        require(
            "allocation first-free hint",
            condition=integer(field(pool, "first_free")) == self.first_free,
        )
        require(
            "allocation first-unused hint",
            condition=integer(field(pool, "first_unused")) == self.first_unused,
        )
        require(
            "allocation growth slots",
            condition=integer(field(pool, "slots")) == self.slots,
        )
        roads = tuple(integer(field(row, "id")) for row in array(field(value, "road")))
        require("allocation road ID membership", condition=roads == self.occupied)
        units = array(field(value, "units"))
        require("recipe exactly one company unit roster", condition=len(units) == 1)
        keys(units[0], {"company", "count", "next"})
        require(
            "allocation unit company",
            condition=integer(field(units[0], "company")) == 0,
        )
        require(
            "allocation unit count",
            condition=integer(field(units[0], "count")) == len(self.occupied),
        )
        require(
            "allocation next unit",
            condition=integer(field(units[0], "next")) == self.next_unit,
        )


def transition(receipt: Json, before: Allocation, after: Allocation) -> None:
    before.check(field(field(receipt, "before"), "allocation"))
    after.check(field(field(receipt, "after"), "allocation"))


def verify(result: Json, case: Case) -> None:
    """Bind the empty-seed sequential-purchase and ordered-sale recipe."""
    empty = Allocation((), 0, 0, 0, 1)
    settings = array(field(result, "prepare_settings"))
    require("allocation setting count", condition=len(settings) == 1)
    transition(settings[0], empty, empty)
    commands = array(field(result, "commands"))
    require(
        "allocation command roster",
        condition=len(commands) == 28 + 2 * case.reservations,
    )
    for command in commands[:26]:
        transition(command, empty, empty)
    for target in range(case.reservations + 1):
        before = Allocation(
            tuple(range(target)), target, target, 512 if target else 0, target + 1
        )
        after = Allocation(
            tuple(range(target + 1)), target + 1, target + 1, 512, target + 2
        )
        command = commands[26 + target]
        returned = integer(
            field(field(field(field(command, "receipt"), "returns"), "exec"), "vehicle")
        )
        require("allocation original returned ID", condition=returned == target)
        transition(command, before, after)
    total = case.reservations + 1
    for released in range(case.reservations):
        before = Allocation(
            tuple(range(released, total)),
            0 if released else total,
            total,
            512,
            1 if released else total + 1,
        )
        after = Allocation(tuple(range(released + 1, total)), 0, total, 512, 1)
        command = commands[27 + case.reservations + released]
        require(
            "allocation actual sale ID",
            condition=integer(
                field(field(field(command, "request"), "command"), "vehicle")
            )
            == released,
        )
        transition(command, before, after)
    sole = Allocation(
        (case.reservations,),
        0 if case.reservations else 1,
        total,
        512,
        1 if case.reservations else 2,
    )
    transition(commands[-1], sole, sole)
