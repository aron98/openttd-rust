"""Real archive producer interleaving with the capacity scanner."""

import os
import subprocess
import sys
from pathlib import Path

import pytest

from scripts import movement_pr_process as monitor
from scripts.movement_ci import archive_controls, artifacts
from scripts.movement_ci.baseline import FileInput
from scripts.movement_ci.storage import archive_lock
from scripts.movement_ci.value import DECODE, field

ROOT = Path(__file__).resolve().parents[2]


def first_seal(output: Path) -> None:
    output.mkdir()
    source = ROOT / "fixtures/road-movement/bus-first.sav"
    for folder in ("sources", "native-inputs", "baseline-inputs"):
        artifacts.retain(output / folder, (FileInput.capture(source),))
    artifacts.seal(output)


@pytest.mark.parametrize("name", ["evidence.tar.gz", "evidence-index.json"])
@pytest.mark.parametrize("surface", ["size", "phase"])
def test_actual_archive_rotation_waits_for_enumerated_file_stat(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, name: str, surface: str
) -> None:
    # Given the real first seal and an actual producer waiting for a scan boundary.
    output = tmp_path / "capture"
    first_seal(output)
    expected = monitor.size(tmp_path)
    program = (
        "import fcntl,os,sys;from pathlib import Path;"
        "from scripts.movement_ci import archive_controls,artifacts;"
        f"output=Path({str(output)!r});"
        "print('ready',flush=True);sys.stdin.readline();"
        "fd=os.open(output,os.O_RDONLY|os.O_DIRECTORY);\n"
        "try:\n fcntl.flock(fd,fcntl.LOCK_EX|fcntl.LOCK_NB)\n"
        "except BlockingIOError:\n print('blocked',flush=True)\n"
        "else:\n print('unlocked',flush=True)\n"
        "finally:\n os.close(fd)\n"
        "archive_controls.run(output);artifacts.seal(output);"
        "archive_controls.verify(output);artifacts.verify(output);"
        "print('sealed',flush=True)"
    )
    original = Path.stat
    scheduled = False
    with subprocess.Popen(
        [sys.executable, "-c", program],
        cwd=ROOT,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    ) as producer:
        assert producer.stdout is not None
        assert producer.stdin is not None
        assert os.read(producer.stdout.fileno(), 64).decode().strip() == "ready"

        def rotate_before_stat(
            path: Path, *, follow_symlinks: bool = True
        ) -> os.stat_result:
            nonlocal scheduled
            if path == output / name and not follow_symlinks and not scheduled:
                scheduled = True
                assert producer.stdin is not None
                assert producer.stdout is not None
                _ = producer.stdin.write("rotate\n")
                producer.stdin.flush()
                state = os.read(producer.stdout.fileno(), 64).decode().strip()
                if state == "unlocked":
                    assert (
                        os.read(producer.stdout.fileno(), 64).decode().strip()
                        == "sealed"
                    )
                assert state == "blocked", (
                    "producer renamed an enumerated retained file"
                )
            return original(path, follow_symlinks=follow_symlinks)

        monkeypatch.setattr(Path, "stat", rotate_before_stat)
        # When scanning, the real producer must wait until every byte was counted.
        if surface == "size":
            observed = monitor.size(tmp_path, archive_parent=output)
            assert observed == expected
        else:
            assert (
                monitor.phase(
                    ROOT,
                    tmp_path,
                    "monitor",
                    [sys.executable, "-c", "import time;time.sleep(0.3)"],
                    monitor.Bounds(0, 0, 10**7, 10, tmp_path, archive_parent=output),
                )
                == 0
            )
            assert (
                field(
                    DECODE((tmp_path / "monitor/result.json").read_text()),
                    "classification",
                )
                == "process_exit"
            )
        stdout, stderr = producer.communicate(timeout=10)
        # Then the original identity is counted once and both seals remain valid.
        assert scheduled
        assert producer.returncode == 0, stderr
        assert stdout.strip() == "sealed"
    archive_controls.verify(output)
    artifacts.verify(output)
    assert monitor.size(tmp_path, archive_parent=output) == sum(
        path.stat().st_size for path in tmp_path.rglob("*") if path.is_file()
    )


@pytest.mark.parametrize("action", ["delete", "symlink"])
def test_archive_names_do_not_exempt_missing_or_linked_retained_data(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, action: str
) -> None:
    # Given a retained archive whose name alone grants no disappearance exemption.
    output = tmp_path / "capture"
    first_seal(output)
    member = output / "evidence.tar.gz"
    original = Path.stat
    changed = False

    def corrupt_before_stat(
        path: Path, *, follow_symlinks: bool = True
    ) -> os.stat_result:
        nonlocal changed
        if path == member and not follow_symlinks and not changed:
            changed = True
            member.unlink()
            if action == "symlink":
                member.symlink_to(tmp_path / "outside")
        return original(path, follow_symlinks=follow_symlinks)

    monkeypatch.setattr(Path, "stat", corrupt_before_stat)
    # When external mutation bypasses coordination, then scanning still fails closed.
    with pytest.raises((FileNotFoundError, monitor.CaptureError)):
        _ = monitor.size(tmp_path, archive_parent=output)
    assert changed
    with archive_lock(output, exclusive=True):
        assert changed


def test_archive_lock_does_not_follow_directory_symlink(tmp_path: Path) -> None:
    destination = tmp_path / "real"
    destination.mkdir()
    alias = tmp_path / "alias"
    alias.symlink_to(destination, target_is_directory=True)
    with (
        pytest.raises(OSError, match=r"directory|symbolic"),
        archive_lock(alias, exclusive=True),
    ):
        pytest.fail("directory symlink was followed")


def test_rotating_archive_bytes_still_trigger_storage_guard(tmp_path: Path) -> None:
    output = tmp_path / "capture"
    first_seal(output)
    assert (
        monitor.phase(
            ROOT,
            tmp_path,
            "monitor",
            [sys.executable, "-c", "import time;time.sleep(10)"],
            monitor.Bounds(0, 0, 1, 10, tmp_path, archive_parent=output),
        )
        == 124
    )
    assert (
        field(DECODE((tmp_path / "monitor/result.json").read_text()), "classification")
        == "storage_guard"
    )
    artifacts.verify(output)


def test_archive_lock_contention_has_bounded_monitor_failure(tmp_path: Path) -> None:
    output = tmp_path / "capture"
    first_seal(output)
    program = (
        "import sys;from pathlib import Path;"
        "from scripts.movement_ci.storage import archive_lock;"
        f"output=Path({str(output)!r});\n"
        "with archive_lock(output,exclusive=True):\n"
        " print('locked',flush=True);sys.stdin.readline()\n"
    )
    with subprocess.Popen(
        [sys.executable, "-c", program],
        cwd=ROOT,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    ) as holder:
        assert holder.stdout is not None
        assert os.read(holder.stdout.fileno(), 64).decode().strip() == "locked"
        try:
            assert (
                monitor.phase(
                    ROOT,
                    tmp_path,
                    "monitor",
                    [sys.executable, "-c", "import time;time.sleep(20)"],
                    monitor.Bounds(0, 0, 10**7, 10, tmp_path, archive_parent=output),
                )
                == 124
            )
        finally:
            _, stderr = holder.communicate("release\n", timeout=5)
        assert holder.returncode == 0, stderr
    receipt = DECODE((tmp_path / "monitor/result.json").read_text())
    assert field(receipt, "classification") == "inspection_error"
    assert field(receipt, "monitor_error") == "archive rotation lock deadline"
    with archive_lock(output, exclusive=True):
        artifacts.verify(output)
