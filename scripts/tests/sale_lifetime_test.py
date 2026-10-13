# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = ["pytest"]
# ///
# Run: uv run --with pytest python -m pytest scripts/tests/sale_lifetime_test.py
"""Sale and reuse incarnation boundaries."""

import copy

import pytest

from scripts.replay_matrix import deterministic
from scripts.sale_saved_state import compare
from scripts.tests.test_purchase_saved_state import plan, results, world
from scripts.world_check_support import Json, WorldCheckError, at, replace


def reuse() -> tuple[Json, Json]:
    request = plan()
    receipt = results()
    sale: Json = {
        "ordinal": 1,
        "op": "command",
        "request": {"mode": "post", "command": {"kind": "sell_vehicle", "vehicle": 34}},
    }
    returned: Json = {
        "ordinal": 1,
        "op": "command",
        "receipt": {
            "exec": {"success": True},
            "result": {"success": True},
            "returns": {"exec": {"kind": "none"}, "result": {"kind": "none"}},
        },
    }
    replace(request, ("actions",), [sale, at(request, ("actions", 0))])
    replace(receipt, ("actions",), [returned, at(receipt, ("actions", 0))])
    replace(
        receipt,
        ("actions", 0, "native_metadata"),
        {
            "sale_before": {"vehicle": {"pool": {"occupied": [34]}}},
            "result": {"sale": {"vehicle": {"pool": {"occupied": []}}}},
        },
    )
    replace(
        receipt,
        ("actions", 1, "native_metadata"),
        {
            "sale_before": {"vehicle": {"pool": {"occupied": []}}},
            "result": {"sale": {"vehicle": {"pool": {"occupied": [34]}}}},
        },
    )
    replace(receipt, ("actions", 0, "receipt", "posted"), True)
    replace(receipt, ("actions", 1, "receipt", "posted"), True)
    return request, receipt


def test_initial_id_is_fresh_only_after_committed_sale_and_build() -> None:
    request: Json
    receipt: Json
    request, receipt = reuse()
    ledger = compare(
        request,
        receipt,
        deterministic(receipt),
        world(99),
        world(99),
        "final",
        world(-1431655766),
        world(0),
    )
    assert at(ledger, ("fresh_initialization", 0, "creation_ordinal")) == 2


@pytest.mark.parametrize(
    "operation",
    ["failed-sale", "estimate-sale", "failed-build", "equal-nonzero", "loaded", "tick"],
)
def test_unproven_lifetime_is_rejected(operation: str) -> None:
    request: Json
    receipt: Json
    request, receipt = reuse()
    rust = world(0)
    if operation == "failed-sale":
        replace(receipt, ("actions", 0, "receipt", "exec", "success"), False)
    if operation == "estimate-sale":
        replace(request, ("actions", 0, "request", "mode"), "estimate")
        replace(receipt, ("actions", 0, "receipt", "exec"), None)
    if operation == "failed-build":
        replace(receipt, ("actions", 1, "receipt", "exec", "success"), False)
    if operation == "equal-nonzero":
        rust = world(48)
    if operation == "loaded":
        request = {"actions": []}
        receipt = {"actions": []}
    if operation == "tick":
        replace(request, ("actions", 0, "op"), "tick")
        replace(receipt, ("actions", 0, "op"), "tick")
    with pytest.raises(WorldCheckError):
        _ = compare(
            request,
            receipt,
            deterministic(receipt),
            world(99),
            world(99),
            "final",
            world(48),
            rust,
        )


def test_sale_target_must_match_native_removal() -> None:
    request, receipt = reuse()
    native = copy.deepcopy(receipt)
    replace(
        native,
        ("actions", 0, "native_metadata"),
        {
            "sale_before": {"vehicle": {"pool": {"occupied": [34]}}},
            "result": {"sale": {"vehicle": {"pool": {"occupied": [34]}}}},
        },
    )
    with pytest.raises(WorldCheckError):
        _ = compare(
            request,
            native,
            deterministic(receipt),
            world(99),
            world(99),
            "final",
            world(48),
            world(0),
        )
