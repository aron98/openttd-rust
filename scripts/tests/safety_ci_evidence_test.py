from copy import deepcopy
from pathlib import Path

import pytest

from scripts.context_ci_support import sources
from scripts.depot_build_archive import bounded_paths, package_raw, verify_archive
from scripts.safety_ci_evidence import compare_case, compare_restoration, require_cases
from scripts.world_check_support import Json, WorldCheckError, replace


def example() -> Json:
    checksum: list[Json] = [0] * 16
    value: dict[str, Json] = {
        "id": "sample",
        "accepted": True,
        "status": 1,
        "unsafe": False,
        "identity": {"grfid": 1, "md5": checksum},
        "grfid": 1,
        "system": False,
        "invalid": False,
        "name": [65],
        "info": None,
        "failure": {"reason": "ReadBounds", "line": 2},
        "decisions": [{"line": 1, "offset": 7, "action": 8, "consumed": 1, "skip": 0}],
    }
    return value


def test_disabled_but_safe_and_accepted_is_preserved() -> None:
    native = example()
    compare_case(native, deepcopy(native))


@pytest.mark.parametrize(
    ("path", "value"),
    [
        (("status",), True),
        (("unsafe",), True),
        (("accepted",), False),
        (("identity", "md5", 0), 1),
        (("failure", "line"), 3),
        (("decisions", 0, "skip"), -1),
        (("decisions", 0, "consumed"), 2),
    ],
)
def test_altered_native_observable_is_rejected(
    path: tuple[str | int, ...], value: Json
) -> None:
    native = example()
    rust = deepcopy(native)
    replace(rust, path, value)
    with pytest.raises(WorldCheckError):
        compare_case(native, rust)


def test_missing_complete_case_set_is_rejected() -> None:
    with pytest.raises(WorldCheckError):
        require_cases(["sample"], ["sample"])


def test_duplicate_case_identity_is_rejected() -> None:
    with pytest.raises(WorldCheckError):
        require_cases(["duplicate"] * 893, ["duplicate"] * 893)


def test_native_restoration_change_is_rejected() -> None:
    raw: Json = {
        "before": {"tick": 0},
        "restored": {"tick": 1},
        "files_before": [],
        "files_restored": [],
    }
    with pytest.raises(WorldCheckError):
        compare_restoration(raw)


def test_absent_restoration_is_rejected() -> None:
    with pytest.raises(WorldCheckError):
        compare_restoration({})


def test_changed_source_pin_is_rejected(tmp_path: Path) -> None:
    _ = (tmp_path / "source.rs").write_text("pub fn changed() {}\n")
    with pytest.raises(WorldCheckError):
        sources(tmp_path, {"sources": {"source.rs": "0" * 64}})


def test_empty_substantive_artifact_is_rejected(tmp_path: Path) -> None:
    _ = (tmp_path / "proof.json").write_bytes(b"")
    with pytest.raises(WorldCheckError):
        _ = bounded_paths(tmp_path, {"proof.json"})


def test_missing_raw_artifact_is_rejected(tmp_path: Path) -> None:
    with pytest.raises(WorldCheckError):
        _ = bounded_paths(tmp_path, {"proof.json"})


def test_archive_readback_rejects_changed_raw_bytes(tmp_path: Path) -> None:
    source = tmp_path / "proof.json"
    _ = source.write_text('{"result": 1}\n')
    package_raw(tmp_path)
    _ = source.write_text('{"result": 2}\n')
    with pytest.raises(WorldCheckError):
        verify_archive(tmp_path)
