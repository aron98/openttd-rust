from __future__ import annotations

import pytest

from scripts.strings_ci_compare import controls
from scripts.strings_ci_evidence import membership
from scripts.world_check_support import Json, WorldCheckError


def test_missing_or_duplicate_case_is_rejected() -> None:
    expected: Json = [{"case": "first"}, {"case": "second"}]
    variations: list[Json] = [
        [{"case": "first"}],
        [{"case": "first"}, {"case": "first"}],
    ]
    for actual in variations:
        with pytest.raises(WorldCheckError):
            membership(actual, expected)


def test_ineffective_altered_event_is_rejected() -> None:
    with pytest.raises(WorldCheckError):
        _ = controls(
            [{"id": 7}], [{"pointer": "/0/id", "altered": 7, "rejected": True}]
        )


def test_field_and_array_mutations_use_actual_comparison() -> None:
    rows: Json = [
        {"pointer": "/0/id", "altered": 8, "rejected": True},
        {"operation": "remove-final-observation", "rejected": True},
        {"operation": "extra-observation", "rejected": True},
        {"operation": "reorder-observations", "other": 1, "rejected": True},
    ]
    assert controls([{"id": 7}, {"id": 9}], rows) == 4


def test_unexecuted_or_duplicate_mutation_is_rejected() -> None:
    row: Json = {"pointer": "/0/id", "altered": 8, "rejected": True}
    variations: list[Json] = [
        [row, row],
        [{"pointer": "/0/id", "altered": 8, "rejected": False}],
    ]
    for rows in variations:
        with pytest.raises(WorldCheckError):
            _ = controls([{"id": 7}], rows)
