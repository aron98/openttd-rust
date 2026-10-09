from pathlib import Path

import pytest

from scripts.context_ci_support import sources
from scripts.depot_build_archive import bounded_paths, package_raw, verify_archive
from scripts.gameplay_foundations import digest, require_test
from scripts.road_slope_ci_evidence import compare_probe
from scripts.world_check_support import Json, WorldCheckError


def test_missing_native_rows_rejects(tmp_path: Path) -> None:
    with pytest.raises(WorldCheckError, match="Incomplete native"):
        compare_probe({"schema_version": 1, "rows": []}, tmp_path)


@pytest.mark.parametrize("slope", [False, 1, 15])
def test_wrong_or_boolean_input_identity_rejects(tmp_path: Path, slope: Json) -> None:
    row: Json = {
        "slope": slope,
        "requested": 0,
        "existing": 0,
        "other": 0,
        "enabled": False,
    }
    rows: list[Json] = [row] * 155648
    with pytest.raises(WorldCheckError, match="tuple membership"):
        compare_probe({"schema_version": 1, "rows": rows}, tmp_path)


def test_duplicate_tuple_rejects_even_with_exact_count(tmp_path: Path) -> None:
    row: Json = {
        "slope": 0,
        "requested": 0,
        "existing": 0,
        "other": 0,
        "enabled": False,
    }
    rows: list[Json] = [row] * 155648
    with pytest.raises(WorldCheckError, match="tuple membership"):
        compare_probe({"schema_version": 1, "rows": rows}, tmp_path)


def test_exact_count_does_not_admit_zero_tests() -> None:
    with pytest.raises(WorldCheckError):
        require_test("test result: ok. 0 passed; 0 failed; 0 ignored;", "slope")


def test_escaped_evidence_symlink_rejects(tmp_path: Path) -> None:
    source = tmp_path / "outside.json"
    _ = source.write_text("{}")
    root = tmp_path / "proof"
    root.mkdir()
    (root / "inside.json").symlink_to(source)
    with pytest.raises(WorldCheckError, match="symlink"):
        _ = bounded_paths(root, {"inside.json"})


def test_source_pin_drift_rejects(tmp_path: Path) -> None:
    path = tmp_path / "slope.rs"
    _ = path.write_text("original")
    layout: Json = {"sources": {"slope.rs": digest(path)}}
    _ = path.write_text("changed")
    with pytest.raises(WorldCheckError, match="source changed"):
        sources(tmp_path, layout)


def test_archive_readback_rejects_raw_corruption(tmp_path: Path) -> None:
    path = tmp_path / "vector.json"
    _ = path.write_text('{"pieces":1}')
    package_raw(tmp_path)
    _ = path.write_text('{"pieces":2}')
    with pytest.raises(WorldCheckError, match="raw evidence changed"):
        verify_archive(tmp_path)
