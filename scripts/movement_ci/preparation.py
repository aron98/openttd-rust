"""Validate the authentic command-only reservation lifecycle and returned identities."""

from __future__ import annotations

from .allocation import verify as verify_allocation
from .protocol import Case, prepare
from .value import Json, array, field, integer, require, same


def successful(value: Json) -> None:
    receipt = field(value, "receipt")
    require(
        "native command posting/gate",
        condition=field(receipt, "posted") is True and field(receipt, "gate") is None,
    )
    for phase in ("test", "exec", "result"):
        require(
            "native command phase failure",
            condition=field(field(receipt, phase), "success") is True,
        )

    phases = [field(receipt, phase) for phase in ("test", "exec", "result")]
    require(
        "command test/exec/result cost disagreement",
        condition=same(phases[0], phases[1]) and same(phases[1], phases[2]),
    )
    cost = integer(field(phases[1], "cost"))
    before = array(field(field(value, "before"), "companies"))
    after = array(field(field(value, "after"), "companies"))
    require("command company roster changed", condition=len(before) == len(after))
    owner_seen = False
    for a, b in zip(before, after, strict=True):
        require("command company identity", condition=field(a, "id") == field(b, "id"))
        if integer(field(a, "id")) == 0:
            owner_seen = True
            require(
                "command money/cost mismatch",
                condition=integer(field(a, "money")) - integer(field(b, "money"))
                == cost,
            )
            require(
                "command fractional finance drift",
                condition=same(field(a, "fraction"), field(b, "fraction"))
                and same(field(a, "loan"), field(b, "loan")),
            )
        else:
            require("unrelated company finance changed", condition=same(a, b))
    require("command company absent", condition=owner_seen)


def returned(value: Json) -> int:
    return integer(
        field(field(field(field(value, "receipt"), "returns"), "exec"), "vehicle")
    )


def verify(result: Json, case: Case) -> None:
    verify_allocation(result, case)
    expected = prepare(case)
    settings = array(field(result, "prepare_settings"))
    require("setting receipt roster", condition=len(settings) == 1)
    successful(settings[0])
    require(
        "original acceleration transition",
        condition=integer(field(settings[0], "value_before")) == 1
        and integer(field(settings[0], "value_after")) == 0,
    )
    require(
        "setting request drift",
        condition=same(
            field(settings[0], "request"), array(field(expected, "prepare_settings"))[0]
        ),
    )
    commands = array(field(result, "commands"))
    require(
        "complete command receipt roster",
        condition=len(commands) == 28 + 2 * case.reservations,
    )
    require(
        "complete command counter",
        condition=integer(field(result, "command_count")) == 29 + 2 * case.reservations,
    )
    requests = array(field(expected, "commands"))
    for index, value in enumerate(commands):
        successful(value)
        require(
            "command action ordering",
            condition=integer(field(value, "action")) == index + 2,
        )
        _ = field(field(value, "before"), "allocation")
        _ = field(field(value, "after"), "allocation")
    for index in range(26):
        require(
            "construction recipe drift",
            condition=same(field(commands[index], "request"), requests[index]),
        )
        require(
            "construction role",
            condition=field(commands[index], "role") == "construction",
        )
    for index in range(case.reservations):
        value = commands[26 + index]
        require(
            "reservation request drift",
            condition=same(field(value, "request"), requests[26]),
        )
        require(
            "reservation identity",
            condition=returned(value) == index
            and field(value, "role") == "reservation",
        )
    target = commands[26 + case.reservations]
    require(
        "target request drift", condition=same(field(target, "request"), requests[26])
    )
    subject = returned(target)
    require(
        "target identity",
        condition=subject == case.reservations
        and field(target, "role") == "target"
        and integer(field(result, "subject")) == subject,
    )
    for index in range(case.reservations):
        value = commands[27 + case.reservations + index]
        request: Json = {
            "company": 0,
            "mode": "post",
            "command": {
                "kind": "sell_vehicle",
                "location": 673,
                "vehicle": index,
                "sell_chain": False,
                "backup_order": False,
                "client_id": 0,
            },
        }
        require(
            "sale request or identity drift",
            condition=same(field(value, "request"), request),
        )
        require("sale role", condition=field(value, "role") == "reservation_release")
    start: Json = {
        "company": 0,
        "mode": "post",
        "command": {"kind": "movement_prepare_start_stop", "vehicle": subject},
    }
    require(
        "StartStop must target actual returned vehicle",
        condition=same(field(commands[-1], "request"), start),
    )
    feasibility = field(result, "purchase_feasibility")
    require(
        "native purchase feasibility",
        condition=field(feasibility, "buildable") is True
        and field(feasibility, "affordable_before_sales") is True,
    )
    require(
        "native purchase count",
        condition=integer(field(feasibility, "purchase_count"))
        == case.reservations + 1,
    )
    cost = integer(field(feasibility, "unit_cost"))
    available = integer(field(feasibility, "available"))
    require(
        "native gross affordability",
        condition=cost > 0 and available >= cost * (case.reservations + 1),
    )
