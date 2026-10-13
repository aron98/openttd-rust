"""Required-coordinate and exact scalar controls for the passive year observer."""

from __future__ import annotations

import unittest
from collections.abc import Callable

from scripts.movement_ci.value import EvidenceError, Json
from scripts.vehicle_year_model import Snapshot, Statistics, probe, snapshot, statistics


def rejects(action: Callable[[], object]) -> None:
    try:
        _ = action()
    except EvidenceError:
        return
    raise AssertionError("malformed native coordinate accepted")


def row() -> Json:
    return {
        "profit_last_year": -1122,
        "profit_last_year_min_age": 0,
        "num_vehicle_min_age": 0,
        "num_vehicle": 1,
        "num_engines": [[146, 1]],
        "autoreplace_defined": False,
        "autoreplace_finished": False,
    }


class StatisticsTests(unittest.TestCase):
    def test_preserves_signed_profit_when_negative(self) -> None:
        given = row()
        actual = statistics(given)
        assert actual.profit_last_year == -1122
        assert actual.engines == ((146, 1),)

    def test_rejects_missing_profit_coordinate(self) -> None:
        given = row()
        assert isinstance(given, dict)
        del given["profit_last_year_min_age"]
        rejects(lambda: statistics(given))

    def test_rejects_bool_as_native_count(self) -> None:
        given = row()
        assert isinstance(given, dict)
        given["num_vehicle"] = True
        rejects(lambda: statistics(given))

    def test_rejects_duplicate_engine_identity(self) -> None:
        given = row()
        assert isinstance(given, dict)
        given["num_engines"] = [[146, 1], [146, 2]]
        rejects(lambda: statistics(given))

    def test_rejects_out_of_range_native_count(self) -> None:
        given = row()
        assert isinstance(given, dict)
        given["num_vehicle_min_age"] = 65536
        rejects(lambda: statistics(given))

    def test_rejects_integer_as_autoreplace_flag(self) -> None:
        given = row()
        assert isinstance(given, dict)
        given["autoreplace_defined"] = 0
        rejects(lambda: statistics(given))


class SnapshotTests(unittest.TestCase):
    def test_rejects_missing_group_coordinate(self) -> None:
        given: Json = {"schema_version": 1, "companies": [], "vehicles": []}
        rejects(lambda: snapshot(given))


def empty_world() -> Json:
    return {
        "schema_version": 1,
        "companies": [],
        "groups": [],
        "vehicles": [],
        "news_queue": 0,
        "context": {
            "current_company": 16,
            "local_company": 0,
            "networking": False,
            "network_server": False,
            "vehicle_income_warn": True,
            "old_vehicle_warn": True,
        },
        "clock": {
            "tick": 1,
            "economy_date": 0,
            "economy_fract": 0,
            "economy_year": 0,
            "calendar_date": 0,
            "calendar_fract": 0,
        },
    }


class NewsQueueTests(unittest.TestCase):
    def test_accepts_empty_process_with_news_queue(self) -> None:
        given = empty_world()
        actual = snapshot(given)
        assert actual.groups == ()
        assert actual.raw == given

    def test_rejects_missing_news_queue_when_world_only_supplied(self) -> None:
        given = empty_world()
        assert isinstance(given, dict)
        del given["news_queue"]
        rejects(lambda: snapshot(given))

    def test_rejects_news_queue_above_native_maximum(self) -> None:
        given = empty_world()
        assert isinstance(given, dict)
        given["news_queue"] = (1 << 10) + 1
        rejects(lambda: snapshot(given))

    def test_rejects_boolean_news_queue_length(self) -> None:
        given = empty_world()
        assert isinstance(given, dict)
        given["news_queue"] = True
        rejects(lambda: snapshot(given))


class ProbeTests(unittest.TestCase):
    def test_accepts_unprofitable_news_probe(self) -> None:
        given: Json = {"company": 0, "string": 761, "profit": -1125}
        assert probe("unprofitable_news", given) == given

    def test_accepts_unprofitable_ai_probe_with_human_owner(self) -> None:
        given: Json = {"company": 0, "is_ai": False}
        assert probe("unprofitable_ai", given) == given

    def test_rejects_missing_profit_in_unprofitable_news(self) -> None:
        given: Json = {"company": 0, "string": 761}
        rejects(lambda: probe("unprofitable_news", given))

    def test_rejects_boolean_company_in_unprofitable_ai(self) -> None:
        given: Json = {"company": True, "is_ai": False}
        rejects(lambda: probe("unprofitable_ai", given))

    def test_rejects_integer_is_ai_in_unprofitable_ai(self) -> None:
        given: Json = {"company": 0, "is_ai": 0}
        rejects(lambda: probe("unprofitable_ai", given))

    def test_rejects_probe_context_on_passive_phase(self) -> None:
        given: Json = {"company": 0, "string": 761, "profit": -1125}
        rejects(lambda: probe("vehicle_year_enter", given))

    def test_accepts_absent_context_on_passive_phase(self) -> None:
        assert probe("vehicle_year_enter", None) is None
