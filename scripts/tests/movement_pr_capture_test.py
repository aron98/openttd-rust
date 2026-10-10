"""Real subprocess and tar boundaries for the PR capture orchestrator."""

import subprocess
import sys
import tarfile
import time
from pathlib import Path

import pytest

from scripts import movement_pr_capture as capture
from scripts import movement_pr_process as helper
from scripts.grf_control_evidence import text
from scripts.language_ci_compare import mapping
from scripts.world_check_support import read_json


def test_real_process_failure_keeps_both_streams(tmp_path: Path) -> None:
    code = "import sys;print('raw-out');print('raw-error',file=sys.stderr);sys.exit(7)"
    result = helper.phase(
        tmp_path,
        tmp_path / "job",
        "probe",
        [sys.executable, "-c", code],
        helper.Bounds(0, 0, 10**7, 10),
    )
    assert result == 7
    assert (tmp_path / "job/probe/stdout.log").read_text() == "raw-out\n"
    assert (tmp_path / "job/probe/stderr.log").read_text() == "raw-error\n"
    receipt = mapping(read_json(tmp_path / "job/probe/result.json"))
    assert receipt["returncode"] == 7
    assert receipt["classification"] == "process_exit"


def test_insufficient_reserve_refuses_before_child_start(tmp_path: Path) -> None:
    marker = tmp_path / "started"
    with pytest.raises(helper.CaptureError, match="reserve"):
        _ = helper.phase(
            tmp_path,
            tmp_path / "job",
            "probe",
            [sys.executable, "-c", f"open({str(marker)!r},'w').close()"],
            helper.Bounds(2**63, 0, 10**7, 10),
        )
    assert not marker.exists()
    assert (tmp_path / "job/probe/invocation.json").is_file()


def test_deadline_stops_only_owned_process_group(tmp_path: Path) -> None:
    result = helper.phase(
        tmp_path,
        tmp_path / "job",
        "probe",
        [sys.executable, "-c", "import time;print('before',flush=True);time.sleep(10)"],
        helper.Bounds(0, 0, 10**7, 0.15),
    )
    assert result != 0
    receipt = mapping(read_json(tmp_path / "job/probe/result.json"))
    assert receipt["classification"] == "deadline"
    assert "before" in (tmp_path / "job/probe/stdout.log").read_text()


def test_failure_tar_preserves_all_raw_and_executable_mode(tmp_path: Path) -> None:
    raw = tmp_path / "raw"
    raw.mkdir()
    executable = raw / "retained"
    _ = executable.write_bytes(b"retained executable bytes")
    executable.chmod(0o755)
    _ = (raw / ".hidden").write_bytes(b"hidden proof")
    archive = tmp_path / "failure.tar.gz"
    helper.pack(raw, archive)
    with tarfile.open(archive) as opened:
        assert {m.name for m in opened if m.isfile()} == {"raw/retained", "raw/.hidden"}
        assert opened.getmember("raw/retained").mode & 0o111 == 0o111
        stream = opened.extractfile("raw/.hidden")
        assert stream is not None
        assert stream.read() == b"hidden proof"


def test_archive_refuses_symlink_escape(tmp_path: Path) -> None:
    raw = tmp_path / "raw"
    raw.mkdir()
    (raw / "escape").symlink_to(tmp_path / "outside")
    with pytest.raises(helper.CaptureError, match="symlink"):
        helper.pack(raw, tmp_path / "failure.tar.gz")


def test_capture_and_target_must_be_fresh_external_siblings(tmp_path: Path) -> None:
    root = tmp_path / "checkout"
    root.mkdir()
    with pytest.raises(helper.CaptureError, match="external"):
        helper.fresh_work(root, root / "job")
    work = tmp_path / "job"
    helper.fresh_work(root, work)
    assert work.is_dir()
    assert not (work / "capture").exists()
    assert not (work / "target").exists()
    with pytest.raises(helper.CaptureError, match="fresh"):
        helper.fresh_work(root, work)


def test_deadline_reaps_owned_descendant(tmp_path: Path) -> None:
    marker = tmp_path / "descendant-finished"
    child = f"import time;time.sleep(0.5);open({str(marker)!r},'w').close()"
    code = (
        f"import subprocess,sys,time;subprocess.Popen([sys.executable,'-c',{child!r}]);"
        "time.sleep(10)"
    )
    assert (
        helper.phase(
            tmp_path,
            tmp_path / "job",
            "probe",
            [sys.executable, "-c", code],
            helper.Bounds(0, 0, 10**7, 0.15),
        )
        == 124
    )
    time.sleep(0.55)
    assert not marker.exists()


def test_raw_cap_stops_process_without_discarding_output(tmp_path: Path) -> None:
    code = (
        "import sys,time;sys.stdout.write('x'*262144);sys.stdout.flush();time.sleep(10)"
    )
    assert (
        helper.phase(
            tmp_path,
            tmp_path / "job",
            "probe",
            [sys.executable, "-c", code],
            helper.Bounds(0, 0, 32768, 10),
        )
        == 124
    )
    assert (tmp_path / "job/probe/stdout.log").stat().st_size == 262144
    assert (
        mapping(read_json(tmp_path / "job/probe/result.json"))["classification"]
        == "storage_guard"
    )


def test_failure_pack_without_setup_completion(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    root = tmp_path / "checkout"
    root.mkdir()
    work = tmp_path / "job"
    (work / "raw/receipts").mkdir(parents=True)
    _ = (work / "raw/receipts/setup-failure.log").write_text("original setup failure")
    monkeypatch.setenv("MOVEMENT_CACHE_OUTCOME", "failure")
    monkeypatch.setenv("PYTHONPATH", str(Path(__file__).resolve().parents[2]))
    monkeypatch.setattr(capture, "FLOOR", 0)
    assert capture.finish(root, work) == 0
    archive = work / "upload/failed-full-raw.tar.gz"
    with tarfile.open(archive) as opened:
        stream = opened.extractfile("raw/receipts/setup-failure.log")
        assert stream is not None
        assert stream.read() == b"original setup failure"
    assert not (work / "raw/receipts/producer-complete").exists()
    assert archive.stat().st_size < helper.archive_bound(work / "raw")


def test_short_lived_process_cannot_escape_final_size_check(tmp_path: Path) -> None:
    assert (
        helper.phase(
            tmp_path,
            tmp_path / "job",
            "probe",
            [sys.executable, "-c", "print('x'*262144)"],
            helper.Bounds(0, 0, 32768, 10),
        )
        == 124
    )


def test_inspection_refusal_records_failure_and_stops_child(tmp_path: Path) -> None:
    link = tmp_path / "job/link"
    code = (
        f"import os,time;os.symlink('outside',{str(link)!r});"
        "print('running',flush=True);time.sleep(0.4)"
    )
    assert (
        helper.phase(
            tmp_path,
            tmp_path / "job",
            "probe",
            [sys.executable, "-c", code],
            helper.Bounds(0, 0, 10**7, 10),
        )
        == 124
    )
    result = mapping(read_json(tmp_path / "job/probe/result.json"))
    assert result["classification"] == "inspection_error"
    assert "symlink" in text(result["monitor_error"])


def test_real_source_and_checkout_identity_are_bound(tmp_path: Path) -> None:
    root = tmp_path / "checkout"
    root.mkdir()
    git = capture.tool("git")
    for command in ([git, "init", "-q"],):
        _ = subprocess.run(command, cwd=root, capture_output=True, check=True)
    member = root / "source.txt"
    _ = member.write_text("source-before")
    _ = subprocess.run(
        [git, "add", "source.txt"], cwd=root, capture_output=True, check=True
    )
    _ = subprocess.run(
        [
            git,
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "fixture",
        ],
        cwd=root,
        capture_output=True,
        check=True,
    )
    work = tmp_path / "job"
    capture.initialize(root, work)
    recorded = (work / "raw/receipts/source.json").read_text()
    assert recorded == capture.source_snapshot(root)
    identity = mapping(read_json(work / "raw/receipts/job.json"))
    assert len(text(identity["head"])) == 40
    assert len(text(identity["tree"])) == 40
    assert identity["required_after_setup"] == 14 * 1024**3 + 256 * 1024**2
    _ = member.write_text("source-after")
    assert recorded != capture.source_snapshot(root)
