from pathlib import Path

import pytest

from scripts.cargo_identity_ci_evidence import invocation_paths
from scripts.cargo_identity_ci_projection import indices, inherited_mask, projection
from scripts.cargo_identity_ci_roster import (
    CASES,
    COMPILER_INPUT,
    GUARD_INPUT,
    INCLUDED_COUNTS,
    LAYOUT,
)
from scripts.cargo_identity_ci_run import prepare, run
from scripts.currency_ci_capture import source_names
from scripts.engine_specs_ci_run import Mode
from scripts.grf_control_evidence import sequence
from scripts.language_ci_compare import mapping
from scripts.world_check_support import Json, WorldCheckError, at, read_json


def test_normal_refuses_unverified_layout(tmp_path: Path) -> None:
    with pytest.raises(WorldCheckError, match="awaits complete paired"):
        _ = prepare(tmp_path, Mode.ADMIT, None, {"verified_native_corpus": False})


def test_capture_refuses_existing_or_contained_output(tmp_path: Path) -> None:
    root = tmp_path / "source"
    root.mkdir()
    for destination in (root / "new", tmp_path):
        with pytest.raises(WorldCheckError, match="fresh absolute external"):
            _ = prepare(root, Mode.CAPTURE, destination, {})
    assert prepare(root, Mode.CAPTURE, tmp_path / "capture", {}).is_dir()


def test_driver_refuses_inherited_subset(
    monkeypatch: pytest.MonkeyPatch, tmp_path: Path
) -> None:
    monkeypatch.setenv("OTTD_CARGO_IDENTITY_CASES", "one-case")
    with pytest.raises(WorldCheckError, match="inherited subset"):
        run(Mode.CAPTURE, tmp_path / "capture")
    assert not (tmp_path / "capture").exists()


def test_phase_scope_accounts_excluded_loader_and_ambient_state() -> None:
    native: Json = {
        "events": [
            {"phase": "after-reset"},
            {"phase": "after-finalize"},
            {"phase": "after-saved-overlay"},
            {"phase": "api-command"},
            {"phase": "property-enter"},
            {"phase": "property-return"},
            {"phase": "road-owner-resolved"},
            {"phase": "api-finish"},
        ]
    }
    assert indices(native, "loader-baseline") == ([0], [1, 2, 3, 4, 5, 6, 7])
    assert indices(native, "api-version-8") == ([3, 4, 5, 6, 7], [0, 1, 2])


def test_projection_rejects_unrecognized_owner_schema() -> None:
    with pytest.raises(WorldCheckError, match="state schema"):
        _ = projection(
            {
                "events": [
                    {"phase": "after-reset", "detail": None, "state": {"unexpected": 1}}
                ]
            },
            "loader-baseline",
        )


@pytest.mark.parametrize("mask", [0, (1 << 64) - 1])
def test_inherited_mask_uses_actual_predecessor_without_recomputation(
    mask: int,
) -> None:
    native: Json = {
        "events": [{"phase": "after-finalize", "state": {"standard_cargo_mask": mask}}]
    }
    assert inherited_mask(native, "api-version-8") == mask


@pytest.mark.parametrize(
    "events",
    [
        [],
        [{"phase": "after-finalize", "state": {"standard_cargo_mask": -1}}],
        [{"phase": "after-finalize", "state": {"standard_cargo_mask": True}}],
    ],
)
def test_inherited_mask_rejects_missing_or_invalid_context(events: list[Json]) -> None:
    with pytest.raises(WorldCheckError, match=r"predecessor|not u64"):
        _ = inherited_mask({"events": events}, "api-version-8")


def test_compiled_corpus_and_source_closure_are_exact() -> None:
    root = Path(__file__).resolve().parents[2]
    definitions = sequence(read_json(root / COMPILER_INPUT))
    assert [at(row, ("name",)) for row in definitions] == list(CASES)
    layout = read_json(root / LAYOUT)
    assert set(mapping(at(layout, ("sources",)))) == set(source_names(root)) | {
        GUARD_INPUT
    }
    assert sum(INCLUDED_COUNTS) == 151
    assert len(sequence(read_json(root / GUARD_INPUT))) == 24


def test_invocation_roster_sorts_serialized_paths(tmp_path: Path) -> None:
    names = [
        "cases/api-version-8/native/invocation.txt",
        "cases/api-version/native/invocation.txt",
        "corruption/copied/invocation.txt",
    ]
    for name in names:
        path = tmp_path / name
        path.parent.mkdir(parents=True)
        _ = path.write_text("binding")
    assert invocation_paths(tmp_path) == sorted(names[:2])
