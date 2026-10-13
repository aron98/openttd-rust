from __future__ import annotations

from pathlib import Path

import pytest

from scripts.gameplay_foundations import digest
from scripts.language_ci_bindings import bound_inputs, manifest_digest, normalized
from scripts.language_ci_compare import compare, control_projection, mutations
from scripts.language_ci_evidence import require_cases
from scripts.world_check_support import Json, WorldCheckError, write_json


def test_comparator_rejects_boolean_in_place_of_native_integer() -> None:
    with pytest.raises(WorldCheckError, match="language observables"):
        compare({"plural": 0}, {"plural": False})


def test_mutation_replays_root_value_through_comparator() -> None:
    assert (
        mutations(7, [{"pointer": "", "before": 7, "after": 8, "rejected": True}]) == 1
    )


def test_unchanged_mutation_is_rejected_despite_success_flag() -> None:
    with pytest.raises(WorldCheckError, match="ineffective"):
        _ = mutations(7, [{"pointer": "", "before": 7, "after": 7, "rejected": True}])


def test_duplicate_mutation_identity_is_rejected() -> None:
    row: Json = {"pointer": "", "before": 7, "after": 8, "rejected": True}
    with pytest.raises(WorldCheckError, match="Duplicate"):
        _ = mutations(7, [row, row])


def test_array_append_control_uses_real_comparator() -> None:
    assert mutations([], [{"operation": "append-null", "rejected": True}]) == 1


def test_missing_case_is_rejected() -> None:
    names = [str(index) for index in range(110)]
    with pytest.raises(WorldCheckError, match="110"):
        require_cases(names, names[:-1])


def test_reordered_case_set_is_rejected() -> None:
    names = [str(index) for index in range(110)]
    with pytest.raises(WorldCheckError, match="110"):
        require_cases(names, names[::-1])


def test_baseline_dynamic_identity_cannot_alias_configured_file() -> None:
    raw: Json = {
        "events": [],
        "overrides": [],
        "files": [
            {"config_grfid": 1, "file_grfid": 3},
            {"config_grfid": 2, "file_grfid": 2},
        ],
    }
    with pytest.raises(WorldCheckError, match="baseline registry identity"):
        _ = control_projection(raw)


def test_mutation_with_wrong_original_value_is_rejected() -> None:
    with pytest.raises(WorldCheckError, match="language observables"):
        _ = mutations(
            {"map": [1]},
            [
                {"pointer": "/map/0", "before": 2, "after": 3, "rejected": True},
            ],
        )


def test_manifest_hash_preserves_bytes_and_order_without_absolute_root(
    tmp_path: Path,
) -> None:
    first = tmp_path / "first"
    second = tmp_path / "second"
    oracle = tmp_path / "oracle"
    before: Json = {"path": str(first / "x"), "raw": [1, 2]}
    same: Json = {"path": str(second / "x"), "raw": [1, 2]}
    changed: Json = {"path": str(second / "x"), "raw": [2, 1]}
    assert manifest_digest(before, (tmp_path, first, oracle)) == manifest_digest(
        same, (tmp_path, second, oracle)
    )
    assert manifest_digest(before, (tmp_path, first, oracle)) != manifest_digest(
        changed, (tmp_path, second, oracle)
    )


def test_changed_input_bytes_rejected_with_unchanged_manifest(tmp_path: Path) -> None:
    manifest: Json = {"raw": [1]}
    write_json(tmp_path / "manifest.json", manifest)
    source = tmp_path / "input.lng"
    _ = source.write_bytes(b"source")
    roots = tmp_path, tmp_path, tmp_path / "oracle"
    layout: Json = {
        "manifest_sha256": manifest_digest(manifest, roots),
        "inputs": {"input.lng": digest(source)},
    }
    _ = source.write_bytes(b"changed")
    with pytest.raises(WorldCheckError, match="input source"):
        bound_inputs(tmp_path, layout, roots)


def test_oracle_inside_ci_workspace_keeps_distinct_identity(tmp_path: Path) -> None:
    oracle = tmp_path / ".reference/snapshot-build/openttd"
    roots = tmp_path, tmp_path / ".artifacts/run/results/case", oracle
    assert normalized(f"-DORACLE={oracle}", roots) == "-DORACLE=$ORACLE"
