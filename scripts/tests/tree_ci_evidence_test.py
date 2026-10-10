from pathlib import Path

import pytest

from scripts.grf_control_evidence import text
from scripts.language_ci_compare import mapping
from scripts.tree_ci_controls import CONTROLS, integer, mutate
from scripts.tree_ci_evidence import membership
from scripts.world_check_support import WorldCheckError, at, read_json


def test_actual_nine_control_vectors() -> None:
    vectors = read_json(
        Path(__file__).resolve().parents[1] / "tree-terrain/control-vectors.json"
    )
    assert set(mapping(at(vectors, ("controls",)))) == set(CONTROLS)
    for name in CONTROLS:
        expected = at(vectors, ("controls", name))
        original = at(
            vectors, ("baselines", text(at(expected, ("baseline",))), "events")
        )
        assert mutate(original, name) == at(expected, ("events",))
        assert original != at(expected, ("events",))


def test_zero_subset_duplicate_and_reordered_membership() -> None:
    required = ["a", "b", "c"]
    membership(required, required)
    for actual in ([], ["a", "b"], ["a", "b", "b"], ["b", "a", "c"]):
        with pytest.raises(WorldCheckError, match="membership"):
            membership(actual, required)


def test_event_integer_is_not_boolean() -> None:
    assert integer(-1000) == -1000
    with pytest.raises(WorldCheckError, match="Boolean"):
        _ = integer(value=False)
