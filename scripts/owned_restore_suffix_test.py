import copy
from typing import Literal

import pytest

from scripts.owned_restore_suffix import VALUE, verify_prepared, verify_return
from scripts.world_check_support import Json, WorldCheckError, replace


def world(value: int) -> Json:
    return {
        "chunks": {
            "VEHS": {
                "records": {
                    "3": {
                        "roadveh": [{"common": [{"round_trip_time": value, "kept": 7}]}]
                    }
                }
            }
        },
        "kept": 9,
    }


def test_declared_one_field_edit() -> None:
    verify_prepared(world(0), world(VALUE))
    verify_prepared(world(-1431655766), world(VALUE))


def test_zero_or_unchanged_input_rejected() -> None:
    with pytest.raises(WorldCheckError, match="declared nonzero"):
        verify_prepared(world(0), world(0))
    with pytest.raises(WorldCheckError, match="ineffective"):
        verify_prepared(world(VALUE), world(VALUE))


def test_unrelated_edit_rejected() -> None:
    changed = world(VALUE)
    assert isinstance(changed, dict)
    changed["kept"] = 10
    with pytest.raises(WorldCheckError, match="another saved field"):
        verify_prepared(world(0), changed)


def receipt() -> Json:
    returned: Json = {"kind": "vehicle", "vehicle": 3}
    return {
        "input": {
            "op": "command",
            "request": {"mode": "post", "command": {"kind": "build_vehicle"}},
        },
        "result": {
            "receipt": {
                "posted": True,
                "exec": {"success": True},
                "result": {"success": True},
                "returns": {"exec": returned, "result": copy.deepcopy(returned)},
            }
        },
    }


def test_prior_successful_return_required() -> None:
    verify_return(receipt())
    changed = receipt()
    assert isinstance(changed, dict)
    changed["input"] = {
        "op": "command",
        "request": {"mode": "estimate", "command": {"kind": "build_vehicle"}},
    }
    with pytest.raises(WorldCheckError, match="prior committed"):
        verify_return(changed)


@pytest.mark.parametrize(
    "kind", ["other-vehicle", "mismatched-return", "failed-result"]
)
def test_prior_return_contradictions(
    kind: Literal["other-vehicle", "mismatched-return", "failed-result"],
) -> None:
    changed = receipt()
    match kind:
        case "other-vehicle":
            replace(changed, ("result", "receipt", "returns", "exec", "vehicle"), 4)
            replace(changed, ("result", "receipt", "returns", "result", "vehicle"), 4)
        case "mismatched-return":
            replace(changed, ("result", "receipt", "returns", "result", "vehicle"), 4)
        case "failed-result":
            replace(
                changed, ("result", "receipt", "result", "success"), replacement=False
            )
    with pytest.raises(WorldCheckError, match="prior committed"):
        verify_return(changed)
