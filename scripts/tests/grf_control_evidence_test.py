from pathlib import Path

import pytest

from scripts.gameplay_foundations import require_test
from scripts.grf_control_archive import archive_files
from scripts.grf_control_evidence import controls, validate
from scripts.world_check_support import WorldCheckError, write_json


def test_zero_execution_is_rejected() -> None:
    with pytest.raises(WorldCheckError, match="not executed"):
        require_test(
            "test result: ok. 0 passed; 0 failed; 0 ignored;",
            "native_load_control_matrix",
        )


def test_empty_substantive_artifact_is_rejected(tmp_path: Path) -> None:
    root = tmp_path / "results"
    root.mkdir()
    (root / "rust.json").touch()
    layout = tmp_path / "layout.json"
    write_json(layout, {"paths": ["rust.json"], "cases": []})
    with pytest.raises(WorldCheckError, match="Empty substantive"):
        _ = validate(root, layout)


def test_partial_case_set_is_rejected(tmp_path: Path) -> None:
    root = tmp_path / "results"
    root.mkdir()
    layout = tmp_path / "layout.json"
    write_json(layout, {"paths": ["missing/rust.json"], "cases": ["missing"]})
    with pytest.raises(WorldCheckError, match="path set differs"):
        _ = validate(root, layout)


def test_ineffective_mutation_is_rejected() -> None:
    with pytest.raises(WorldCheckError, match="ineffective"):
        _ = controls(
            {"status": 1},
            [
                {
                    "pointer": "/status",
                    "before": 1,
                    "after": 1,
                    "comparator_rejected": True,
                }
            ],
        )


def test_false_comparator_receipt_is_rejected() -> None:
    with pytest.raises(WorldCheckError, match="did not reject"):
        _ = controls(
            {"status": 1},
            [
                {
                    "pointer": "/status",
                    "before": 1,
                    "after": 2,
                    "comparator_rejected": False,
                }
            ],
        )


def test_altered_receipt_must_match_original_value() -> None:
    with pytest.raises(WorldCheckError, match="ineffective"):
        _ = controls(
            {"status": 1},
            [
                {
                    "pointer": "/status",
                    "before": 3,
                    "after": 2,
                    "comparator_rejected": True,
                }
            ],
        )


def test_packager_refuses_empty_evidence(tmp_path: Path) -> None:
    artifact = tmp_path / "comparison.txt"
    artifact.touch()
    with pytest.raises(WorldCheckError, match="Empty substantive"):
        archive_files(tmp_path, [artifact])


def test_packager_retains_and_indexes_exact_bytes(tmp_path: Path) -> None:
    artifact = tmp_path / "comparison.txt"
    _ = artifact.write_text("PASS native and Rust exact comparison\n")
    archive_files(tmp_path, [artifact])
    assert artifact.read_text() == "PASS native and Rust exact comparison\n"
    assert (tmp_path / "evidence.tar.gz").stat().st_size > 0
    assert (tmp_path / "evidence-index.json").stat().st_size > 0


def test_discriminating_control_is_accepted() -> None:
    assert (
        controls(
            {"status": 1},
            [
                {
                    "pointer": "/status",
                    "before": 1,
                    "after": 2,
                    "comparator_rejected": True,
                }
            ],
        )
        == 1
    )
