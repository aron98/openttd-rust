import os
import sys
from pathlib import Path

import pytest

from scripts import movement_pr_process as helper
from scripts.grf_control_evidence import text
from scripts.language_ci_compare import mapping
from scripts.world_check_support import read_json


@pytest.mark.parametrize(("folder", "expected"), [("target", 0), ("raw", 124)])
def test_only_cargo_atomic_removal_is_tolerated(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, folder: str, expected: int
) -> None:
    work = tmp_path / "job"
    target = work / "target"
    target.mkdir(parents=True)
    (work / folder).mkdir(exist_ok=True)
    member = work / folder / "compiler.rmeta"
    _ = member.write_bytes(b"compiler intermediate")
    monkeypatch.setenv("CARGO_TARGET_DIR", str(target))
    original = Path.stat
    observations = 0

    def remove_during_stat(
        path: Path, *, follow_symlinks: bool = True
    ) -> os.stat_result:
        nonlocal observations
        if path == member:
            observations += 1
            if observations == 3:
                path.unlink()
        return original(path, follow_symlinks=follow_symlinks)

    monkeypatch.setattr(Path, "stat", remove_during_stat)
    result = helper.phase(
        tmp_path,
        work,
        "probe",
        [sys.executable, "-c", "import time;time.sleep(0.4)"],
        helper.Bounds(0, 0, 10**7, 10, work),
    )
    assert observations >= 3
    assert not member.exists()
    assert result == expected


def test_configured_target_symlink_is_still_refused(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    work = tmp_path / "job"
    target = work / "target"
    target.mkdir(parents=True)
    (target / "link").symlink_to(tmp_path / "outside")
    monkeypatch.setenv("CARGO_TARGET_DIR", str(target))
    assert (
        helper.phase(
            tmp_path,
            work,
            "probe",
            [sys.executable, "-c", "import time;time.sleep(0.4)"],
            helper.Bounds(0, 0, 10**7, 10, work),
        )
        == 124
    )
    result = mapping(read_json(work / "probe/result.json"))
    assert result["classification"] == "inspection_error"
    assert "symlink" in text(result["monitor_error"])
