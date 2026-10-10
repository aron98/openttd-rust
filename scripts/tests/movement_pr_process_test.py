import os
import select
import signal
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


def test_deadline_stops_term_resistant_child_after_leader_exits(tmp_path: Path) -> None:
    work = tmp_path / "job"
    work.mkdir()
    fifo = tmp_path / "child-output"
    os.mkfifo(fifo)
    ready = tmp_path / "child-ready.json"
    marker = tmp_path / "child-wrote-after-return"
    child = (
        "import os,signal,time,json;"
        "signal.signal(signal.SIGTERM,signal.SIG_IGN);"
        f"fd=os.open({str(fifo)!r},os.O_WRONLY);"
        f"open({str(ready)!r},'w').write(json.dumps({{'pid':os.getpid()}}));"
        "print('ready',flush=True);time.sleep(10);"
        f"open({str(marker)!r},'w').write('survived')"
    )
    parent = (
        "import subprocess,sys,time;"
        f"p=subprocess.Popen([sys.executable,'-c',{child!r}],stdout=subprocess.PIPE);"
        "p.stdout.readline();time.sleep(10)"
    )
    reader = os.open(fifo, os.O_RDONLY | os.O_NONBLOCK)
    stopped = False
    try:
        result = helper.phase(
            tmp_path,
            work,
            "deadline",
            [sys.executable, "-c", parent],
            helper.Bounds(0, 0, 10**7, 1, work),
        )
        assert result == 124
        assert ready.is_file()
        readable, _, _ = select.select([reader], [], [], 2)
        stopped = bool(readable) and os.read(reader, 1) == b""
        assert stopped, "descendant retains its pipe after the leader exits"
        assert not marker.exists()
    finally:
        os.close(reader)
        if not stopped and ready.is_file():
            pid = mapping(read_json(ready))["pid"]
            assert isinstance(pid, int)
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                assert marker.exists(), "child exited before test cleanup"
