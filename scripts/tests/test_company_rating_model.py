"""Exact native rating coordinates and malformed-boundary controls."""

from __future__ import annotations

import unittest

from scripts.company_rating_model import company, economy, score_rows, snapshot
from scripts.movement_ci.value import Json, array
from scripts.tests import company_rating_fixture as fixture


def rows() -> list[Json]:
    result: list[Json] = [
        {"owner": owner, "parts": list[Json]([0] * 10)} for owner in range(15)
    ]
    return result


class RatingTests(unittest.TestCase):
    def test_preserves_inactive_negative_cache_when_observed(self) -> None:
        given = rows()
        given[14] = {"owner": 14, "parts": [0, 0, 0, 0, -123, 0, 0, 0, -999, 0]}
        actual = score_rows(given)
        assert actual[14][4] == -123
        assert actual[14][8] == -999

    def test_rejects_missing_owner_when_array_short(self) -> None:
        given = rows()[:-1]
        with fixture.rejects():
            _ = score_rows(given)

    def test_rejects_duplicate_owner_when_array_full(self) -> None:
        given = rows()
        given[14] = given[0]
        with fixture.rejects():
            _ = score_rows(given)

    def test_rejects_missing_component_when_total_absent(self) -> None:
        given = rows()
        given[0] = {"owner": 0, "parts": list[Json]([0] * 9)}
        with fixture.rejects():
            _ = score_rows(given)

    def test_rejects_boolean_when_integer_expected(self) -> None:
        given = rows()
        given[0] = {"owner": 0, "parts": list[Json]([True]) + list[Json]([0] * 9)}
        with fixture.rejects():
            _ = score_rows(given)

    def test_rejects_overflow_when_signed64_exceeded(self) -> None:
        given = rows()
        given[0] = {"owner": 0, "parts": list[Json]([2**63]) + list[Json]([0] * 9)}
        with fixture.rejects():
            _ = score_rows(given)


class SnapshotTests(unittest.TestCase):
    def test_accepts_complete_empty_process_when_all_slots_present(self) -> None:
        given = fixture.snapshot()
        actual = snapshot(given)
        assert len(actual.scores) == 15
        assert actual.companies == ()

    def test_rejects_missing_score_cache_when_world_only_supplied(self) -> None:
        given = fixture.snapshot()
        del given["scores"]
        with fixture.rejects():
            _ = snapshot(given)

    def test_rejects_changed_weights_when_native_pin_differs(self) -> None:
        given = fixture.snapshot()
        info = array(given["score_info"])
        info[0] = {"id": 0, "needed": 121, "weight": 100}
        with fixture.rejects():
            _ = snapshot(given)

    def test_rejects_company_when_group_owner_absent(self) -> None:
        given = fixture.snapshot()
        given["companies"] = [
            {
                "id": 0,
                "valid_history": 0,
                "block_preview": 0,
                "money": 0,
                "loan": 0,
                "hq_tile": 2**32 - 1,
                "current": fixture.economy(),
                "history": [fixture.economy() for _ in range(24)],
            }
        ]
        with fixture.rejects():
            _ = snapshot(given)

    def test_rejects_short_history_when_valid_count_is_zero(self) -> None:
        given: Json = {
            "id": 0,
            "valid_history": 0,
            "block_preview": 0,
            "money": 0,
            "loan": 0,
            "hq_tile": 2**32 - 1,
            "current": fixture.economy(),
            "history": [],
        }
        with fixture.rejects():
            _ = company(given)

    def test_rejects_missing_cargo_slot_when_zero_history(self) -> None:
        given = fixture.economy()
        assert isinstance(given, dict)
        given["delivered_cargo"] = list[Json]([0] * 63)
        with fixture.rejects():
            _ = economy(given)
