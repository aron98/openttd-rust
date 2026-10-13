from pathlib import Path

import pytest

from scripts.owned_restore_controls import mutate
from scripts.owned_restore_evidence import validate
from scripts.owned_restore_freshness import relocated_stamp
from scripts.owned_restore_run import RestoreRun
from scripts.world_check_support import Json, WorldCheckError


def test_zero_case_admission(tmp_path: Path) -> None:
    (tmp_path / "results/cases").mkdir(parents=True)
    job = RestoreRun(tmp_path, tmp_path, tmp_path / "native", {}, {"cases": []})
    with pytest.raises(WorldCheckError, match="29-case membership"):
        _ = validate(job)


def test_subset_and_extra_case_admission(tmp_path: Path) -> None:
    case_root = tmp_path / "results/cases"
    case_root.mkdir(parents=True)
    entries: list[Json] = [{"name": f"case-{index}"} for index in range(29)]
    for index in range(28):
        (case_root / f"case-{index}").mkdir()
    job = RestoreRun(tmp_path, tmp_path, tmp_path / "native", {}, {"cases": entries})
    with pytest.raises(WorldCheckError, match="29-case membership"):
        _ = validate(job)
    (case_root / "case-28").mkdir()
    (case_root / "unlisted").mkdir()
    with pytest.raises(WorldCheckError, match="29-case membership"):
        _ = validate(job)


def test_semantic_mutations_change_only_target() -> None:
    documents: Json = {"actual": {"a": {"value": 4, "keep": 8}}}
    before = mutate(
        documents,
        {"document": "actual", "kind": "replace", "path": ["a", "value"], "value": 9},
    )
    assert before == 4
    assert documents == {"actual": {"a": {"value": 9, "keep": 8}}}
    with pytest.raises(WorldCheckError, match="ineffective"):
        _ = mutate(
            documents,
            {
                "document": "actual",
                "kind": "replace",
                "path": ["a", "value"],
                "value": 9,
            },
        )


def test_membership_mutations_have_explicit_preconditions() -> None:
    documents: Json = {"actual": {"a": 1}}
    assert (
        mutate(documents, {"document": "actual", "kind": "remove", "path": ["a"]}) == 1
    )
    with pytest.raises(WorldCheckError, match="absent"):
        _ = mutate(documents, {"document": "actual", "kind": "remove", "path": ["a"]})
    assert (
        mutate(
            documents, {"document": "actual", "kind": "add", "path": ["a"], "value": 2}
        )
        is None
    )
    with pytest.raises(WorldCheckError, match="already exists"):
        _ = mutate(
            documents, {"document": "actual", "kind": "add", "path": ["a"], "value": 2}
        )


def test_native_stamp_relocation_preserves_hashes_and_source_lines() -> None:
    original = Path("/immutable/openttd")
    copied = Path("/control/openttd")
    sha = "a" * 64
    tail = "b" * 64 + "  reference/order_fixture.hpp\n"
    stamp = f"{sha}  {original}\n{tail}"
    assert relocated_stamp(stamp, original, copied, sha) == f"{sha}  {copied}\n{tail}"
    for invalid in (stamp.replace(sha, "c" * 64), stamp + stamp, tail + stamp):
        with pytest.raises(WorldCheckError, match="Native source stamp"):
            _ = relocated_stamp(invalid, original, copied, sha)
