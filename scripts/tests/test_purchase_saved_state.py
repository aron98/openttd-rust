import copy
import unittest

from scripts.purchase_saved_state import compare, compare_fields
from scripts.world_check_support import Json, WorldCheckError, at, replace


def world(duration: int) -> Json:
    common: Json = {
        "orders": 0,
        "next": 0,
        "next_shared": 0,
        "cur_speed": 0,
        "tick_counter": 0,
        "current_order.type": 0,
        "current_order.flags": 0,
        "cur_real_order_index": 0,
        "cur_implicit_order_index": 0,
        "depot_unbunching_last_departure": 0,
        "depot_unbunching_next_departure": 0,
        "subtype": 1,
        "vehstatus": 11,
        "round_trip_time": duration,
    }
    return {
        "chunks": {
            "VEHS": {
                "records": {"34": {"roadveh": [{"state": 254, "common": [common]}]}}
            }
        }
    }


def plan() -> Json:
    return {
        "actions": [
            {
                "ordinal": 2,
                "op": "command",
                "request": {
                    "mode": "post",
                    "command": {"kind": "build_vehicle"},
                },
            }
        ]
    }


def results() -> Json:
    returned: Json = {"kind": "vehicle", "vehicle": 34}
    return {
        "actions": [
            {
                "ordinal": 2,
                "op": "command",
                "receipt": {
                    "exec": {"success": True},
                    "result": {"success": True},
                    "returns": {"exec": returned, "result": copy.deepcopy(returned)},
                },
            }
        ]
    }


EMPTY: Json = {"chunks": {"VEHS": {"records": {}}}}


class PurchaseSavedStateTests(unittest.TestCase):
    def test_original_allocator_value_is_recorded_without_mutation(self) -> None:
        original = world(-1431655766)
        unchanged = copy.deepcopy(original)
        ledger = compare(
            plan(), results(), results(), EMPTY, EMPTY, "final", original, world(0)
        )
        self.assertEqual(original, unchanged)
        self.assertEqual(
            at(ledger, ("fresh_initialization", 0, "native_value")), -1431655766
        )

    def test_equal_nonzero_fresh_values_are_rejected(self) -> None:
        with self.assertRaises(WorldCheckError):
            _ = compare(
                plan(),
                results(),
                results(),
                EMPTY,
                EMPTY,
                "final",
                world(48),
                world(48),
            )

    def test_loaded_duration_remains_strict_after_resume(self) -> None:
        loaded = world(48)
        empty_plan: Json = {"actions": []}
        with self.assertRaises(WorldCheckError):
            _ = compare(
                empty_plan,
                empty_plan,
                empty_plan,
                loaded,
                loaded,
                "final",
                loaded,
                world(0),
            )

    def test_result_scalar_types_cannot_forge_receipt_equality(self) -> None:
        rust = results()
        replace(rust, ("actions", 0, "receipt", "exec", "success"), 1)
        with self.assertRaises(WorldCheckError):
            _ = compare(
                plan(), results(), rust, EMPTY, EMPTY, "final", world(48), world(0)
            )

    def test_boolean_cannot_stand_in_for_zero_duration(self) -> None:
        rust = world(0)
        replace(
            rust,
            (
                "chunks",
                "VEHS",
                "records",
                "34",
                "roadveh",
                0,
                "common",
                0,
                "round_trip_time",
            ),
            False,
        )
        with self.assertRaises(WorldCheckError):
            _ = compare(
                plan(), results(), results(), EMPTY, EMPTY, "final", world(0), rust
            )

    def test_default_structural_comparison_has_no_exception(self) -> None:
        with self.assertRaises(WorldCheckError):
            compare_fields(world(48), world(0), set())

    def test_absent_field_cannot_be_admitted(self) -> None:
        native = world(48)
        replace(native, ("chunks", "VEHS", "records", "34", "roadveh", 0, "common"), [])
        with self.assertRaises(WorldCheckError):
            _ = compare(
                plan(), results(), results(), EMPTY, EMPTY, "final", native, world(0)
            )
