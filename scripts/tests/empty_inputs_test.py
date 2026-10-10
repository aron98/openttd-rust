from __future__ import annotations

import hashlib
from pathlib import Path

import pytest

from scripts.depot_build_archive import bounded_paths, package_raw
from scripts.grf_control_archive import archive_files
from scripts.world_check_support import WorldCheckError


def empty_input(root: Path) -> frozenset[tuple[str, str]]:
    _ = (root / "input.lng").write_bytes(b"")
    return frozenset({("input.lng", hashlib.sha256(b"").hexdigest())})


@pytest.mark.parametrize("surface", ["bounded", "archive", "package"])
def test_default_callers_reject_empty_input(tmp_path: Path, surface: str) -> None:
    _ = empty_input(tmp_path)
    with pytest.raises(WorldCheckError, match="Empty substantive"):
        default_surface(tmp_path, surface)


def default_surface(root: Path, surface: str) -> None:
    match surface:
        case "bounded":
            _ = bounded_paths(root, {"input.lng"})
        case "archive":
            archive_files(root, [root / "input.lng"])
        case "package":
            package_raw(root)
        case _:
            pytest.fail("Unknown test surface")


def test_exact_empty_input_round_trips_through_real_archive(tmp_path: Path) -> None:
    allowance = empty_input(tmp_path)
    package_raw(tmp_path, empty_inputs=allowance)
    assert (tmp_path / "evidence-index.json").is_file()


@pytest.mark.parametrize("kind", ["unused", "wrong-hash", "nonempty", "other-empty"])
def test_allowance_does_not_admit_other_or_changed_evidence(
    tmp_path: Path, kind: str
) -> None:
    allowance = empty_input(tmp_path)
    match kind:
        case "unused":
            allowance = frozenset({("absent", hashlib.sha256(b"").hexdigest())})
        case "wrong-hash":
            allowance = frozenset({("input.lng", "0" * 64)})
        case "nonempty":
            _ = (tmp_path / "input.lng").write_bytes(b"changed")
        case "other-empty":
            _ = (tmp_path / "observation.json").write_bytes(b"")
        case _:
            pytest.fail("Unknown test mutation")
    with pytest.raises(WorldCheckError):
        package_raw(tmp_path, empty_inputs=allowance)
