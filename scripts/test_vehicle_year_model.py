"""Required-coordinate and exact scalar controls for the passive year observer."""

# ruff: noqa: PT009, PT027
# Plain stdlib discovery is required for this source parser.
from __future__ import annotations

import unittest

from scripts.movement_ci.value import EvidenceError, Json
from scripts.vehicle_year_model import snapshot, statistics


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
        self.assertEqual(actual.profit_last_year, -1122)
        self.assertEqual(actual.engines, ((146, 1),))

    def test_rejects_missing_profit_coordinate(self) -> None:
        given = row()
        assert isinstance(given, dict)
        del given["profit_last_year_min_age"]
        with self.assertRaises(EvidenceError):
            _ = statistics(given)

    def test_rejects_bool_as_native_count(self) -> None:
        given = row()
        assert isinstance(given, dict)
        given["num_vehicle"] = True
        with self.assertRaises(EvidenceError):
            _ = statistics(given)

    def test_rejects_duplicate_engine_identity(self) -> None:
        given = row()
        assert isinstance(given, dict)
        given["num_engines"] = [[146, 1], [146, 2]]
        with self.assertRaises(EvidenceError):
            _ = statistics(given)

    def test_rejects_out_of_range_native_count(self) -> None:
        given = row()
        assert isinstance(given, dict)
        given["num_vehicle_min_age"] = 65536
        with self.assertRaises(EvidenceError):
            _ = statistics(given)

    def test_rejects_integer_as_autoreplace_flag(self) -> None:
        given = row()
        assert isinstance(given, dict)
        given["autoreplace_defined"] = 0
        with self.assertRaises(EvidenceError):
            _ = statistics(given)


class SnapshotTests(unittest.TestCase):
    def test_rejects_missing_group_coordinate(self) -> None:
        given: Json = {"schema_version": 1, "companies": [], "vehicles": []}
        with self.assertRaises(EvidenceError):
            _ = snapshot(given)
