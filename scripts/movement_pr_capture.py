# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# CI: python3 -m scripts.movement_pr_capture init|run|finish WORK
"""PR-only full capture with owned process guards and lossless failure transport."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path
from typing import Final

from scripts.movement_pr_process import (
    Bounds,
    CaptureError,
    archive_bound,
    digest,
    fresh_work,
    pack,
    phase,
)

GIB: Final = 1024**3
FLOOR: Final = GIB
PROOF: Final = 6 * GIB
CARGO: Final = GIB
UPLOAD: Final = PROOF + 256 * 1024**2
LOGS: Final = 64 * 1024**2


def tool(name: str) -> str:
    path = shutil.which(name)
    if path is None:
        raise CaptureError("missing tool: " + name)
    return path


def source_snapshot(root: Path) -> str:
    names = (
        subprocess.check_output([tool("git"), "ls-files", "-z"], cwd=root)
        .decode()
        .split("\0")
    )
    files = {
        name: {
            "sha256": digest(root / name),
            "mode": oct((root / name).stat().st_mode & 0o777),
        }
        for name in names
        if name
    }
    return json.dumps(files, indent=2) + "\n"


def existing_work(root: Path, work: Path) -> None:
    if not work.is_absolute() or work.is_relative_to(root) or work.resolve() != work:
        raise CaptureError("external job directory required")


def initialize(root: Path, work: Path) -> None:
    if shutil.disk_usage(work.parent).free < FLOOR:
        raise CaptureError("initial free floor")
    fresh_work(root, work)
    receipts = work / "raw/receipts"
    receipts.mkdir(parents=True)
    _ = (receipts / "source.json").write_text(source_snapshot(root))
    head = subprocess.check_output(
        [tool("git"), "rev-parse", "HEAD"], cwd=root, text=True
    ).strip()
    tree = subprocess.check_output(
        [tool("git"), "rev-parse", "HEAD^{tree}"], cwd=root, text=True
    ).strip()
    fields = [
        "GITHUB_RUN_ID",
        "GITHUB_RUN_ATTEMPT",
        "GITHUB_SHA",
        "GITHUB_REF",
        "GITHUB_EVENT_NAME",
        "RUNNER_OS",
        "RUNNER_ARCH",
        "ImageOS",
        "ImageVersion",
        "MOVEMENT_PR_HEAD",
    ]
    _ = (receipts / "job.json").write_text(
        json.dumps(
            {
                "head": head,
                "tree": tree,
                "root": str(root),
                "work": str(work),
                "environment": {key: os.environ.get(key) for key in fields},
                "free_before_setup": shutil.disk_usage(work).free,
                "required_after_setup": FLOOR + PROOF + CARGO + UPLOAD,
            },
            indent=2,
        )
        + "\n"
    )


def produce(root: Path, work: Path) -> int:
    existing_work(root, work)
    receipts = work / "raw/receipts"
    if (
        not (receipts / "job.json").is_file()
        or (work / "target").exists()
        or (work / "raw/capture").exists()
    ):
        raise CaptureError("initialized job with fresh target/capture required")
    _ = os.environ.pop("REFERENCE_SOURCE", None)
    os.environ.update(
        CARGO_TARGET_DIR=str(work / "target"),
        CARGO_INCREMENTAL="0",
        CARGO_BUILD_JOBS="1",
        CARGO_PROFILE_DEV_DEBUG="0",
        CARGO_PROFILE_TEST_DEBUG="0",
        PYTHONDONTWRITEBYTECODE="1",
        OTTD_GRF_ORACLE=str(root / ".reference/snapshot-build/openttd"),
    )
    environment = {
        key: value
        for key, value in os.environ.items()
        if key
        in {
            "CARGO_TARGET_DIR",
            "CARGO_INCREMENTAL",
            "CARGO_BUILD_JOBS",
            "CARGO_PROFILE_DEV_DEBUG",
            "CARGO_PROFILE_TEST_DEBUG",
            "CCACHE_DIR",
            "CCACHE_BASEDIR",
            "CCACHE_COMPILERCHECK",
            "CCACHE_MAXSIZE",
            "CCACHE_SLOPPINESS",
            "REFERENCE_COMPILER_LAUNCHER",
            "REFERENCE_DISABLE_PCH",
            "CC",
            "CXX",
            "JOBS",
            "OTTD_GRF_ORACLE",
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
        }
    }
    _ = (receipts / "producer-environment.json").write_text(
        json.dumps(environment, indent=2) + "\n"
    )
    steps = [
        (
            "native-setup",
            ["bash", "scripts/setup-snapshot-reference.sh"],
            Bounds(FLOOR + PROOF + CARGO + UPLOAD, 0, LOGS, 5400, work / "raw"),
        ),
        (
            "capture",
            [
                sys.executable,
                "scripts/check-road-movement-ci.py",
                "--capture",
                str(work / "raw/capture"),
            ],
            Bounds(FLOOR + UPLOAD, PROOF + CARGO, PROOF + CARGO + LOGS, 5400, work),
        ),
        (
            "verify",
            [
                sys.executable,
                "scripts/check-road-movement-ci.py",
                "--verify",
                str(work / "raw/capture"),
            ],
            Bounds(FLOOR + UPLOAD, 0, PROOF + CARGO + LOGS, 1800, work),
        ),
    ]
    for name, argv, bounds in steps:
        if source_snapshot(root) != (receipts / "source.json").read_text():
            raise CaptureError("actual PR source changed")
        result = phase(root, receipts, name, argv, bounds)
        if result != 0:
            return result
    if source_snapshot(root) != (receipts / "source.json").read_text():
        raise CaptureError("actual PR source changed")
    _ = (receipts / "producer-complete").write_text(
        "Full --capture and live --verify passed; normal gate unchanged.\n"
    )
    return 0


def finish(root: Path, work: Path) -> int:
    existing_work(root, work)
    receipts = work / "raw/receipts"
    receipts.mkdir(parents=True, exist_ok=True)
    _ = (receipts / "workflow-outcomes.json").write_text(
        json.dumps(
            {
                key: os.environ.get(key)
                for key in [
                    "MOVEMENT_INIT_OUTCOME",
                    "MOVEMENT_CACHE_OUTCOME",
                    "MOVEMENT_PRODUCER_OUTCOME",
                ]
            },
            indent=2,
        )
        + "\n"
    )
    success = (receipts / "producer-complete").is_file()
    source = receipts if success else work / "raw"
    archive = (
        work
        / "upload"
        / ("job-receipts.tar.gz" if success else "failed-full-raw.tar.gz")
    )
    needed = archive_bound(source)
    if needed > UPLOAD:
        raise CaptureError("lossless archive reserve exceeded")
    result = phase(
        root,
        work / "archive-monitor",
        "pack",
        [
            sys.executable,
            "-m",
            "scripts.movement_pr_capture",
            "pack",
            str(source),
            str(archive),
        ],
        Bounds(FLOOR, needed, UPLOAD, 1800, work / "upload"),
    )
    if result != 0:
        return result
    _ = (work / "upload/transport.json").write_text(
        json.dumps(
            {
                "status": "success" if success else "failure",
                "archive": archive.name,
                "sha256": digest(archive),
                "bytes": archive.stat().st_size,
                "full_corpus_archive": str(work / "raw/capture/evidence.tar.gz")
                if success
                else None,
            },
            indent=2,
        )
        + "\n"
    )
    return 0


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    match sys.argv[1:]:
        case ["init", directory]:
            initialize(root, Path(directory))
            return 0
        case ["run", directory]:
            return produce(root, Path(directory))
        case ["finish", directory]:
            return finish(root, Path(directory))
        case ["pack", source, archive]:
            pack(Path(source), Path(archive))
            return 0
        case _:
            raise CaptureError("usage: movement_pr_capture.py init|run|finish WORK")


if __name__ == "__main__":
    raise SystemExit(main())
