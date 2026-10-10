# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Imported by movement_pr_capture.py; tested with pytest.
"""Bound only owned capture subprocesses and preserve raw tar bytes/modes."""

from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import signal
import stat
import subprocess
import tarfile
import time
from collections.abc import Iterator
from contextlib import suppress
from dataclasses import dataclass
from pathlib import Path

from scripts.movement_ci.protocol import CASES
from scripts.movement_ci.refusals import MUTATIONS


class CaptureError(RuntimeError):
    """A capture orchestration boundary was refused."""


@dataclass(frozen=True, slots=True)
class Bounds:
    floor: int
    reserve: int
    cap: int
    seconds: float
    measure: Path | None = None
    replay_parents: tuple[Path, ...] = ()


def size(
    path: Path,
    *,
    transient: Path | None = None,
    replay_parents: tuple[Path, ...] = (),
) -> int:
    """Count live bytes; only explicitly owned scratch directories may disappear."""
    for parent in replay_parents:
        if (
            not parent.is_absolute()
            or parent.resolve() != parent
            or parent == path
            or not parent.is_relative_to(path)
        ):
            raise CaptureError("replay parent must be canonical inside measured job")
    total = 0
    for member, removable in _members(path, transient, replay_parents):
        try:
            observed = member.lstat()
        except FileNotFoundError:
            if removable:
                continue
            raise
        if stat.S_ISLNK(observed.st_mode):
            raise CaptureError("symlink in job evidence")
        if stat.S_ISREG(observed.st_mode):
            total += observed.st_size
    return total


def _members(
    path: Path, transient: Path | None, replay_parents: tuple[Path, ...]
) -> Iterator[tuple[Path, bool]]:
    pending = [(path, False)] if path.exists() else []
    while pending:
        directory, disposable = pending.pop()
        try:
            entries = os.scandir(directory)
        except FileNotFoundError:
            if disposable:
                continue
            raise
        with entries:
            for entry in entries:
                member = Path(entry.path)
                is_directory = entry.is_dir(follow_symlinks=False)
                removable = (
                    disposable
                    or (transient is not None and member.is_relative_to(transient))
                    or (
                        directory in replay_parents
                        and re.fullmatch(r"\.ottd-replay-[A-Za-z0-9]{6}", entry.name)
                        is not None
                        and is_directory
                    )
                )
                yield member, removable
                if is_directory:
                    pending.append((member, removable))


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def fresh_work(root: Path, work: Path) -> None:
    if not work.is_absolute() or work.is_relative_to(root) or work.resolve() != work:
        raise CaptureError("fresh external job directory required")
    if work.exists():
        raise CaptureError("fresh job directory required")
    work.mkdir(parents=True)


def stop_owned(process: subprocess.Popen[bytes]) -> None:
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        return
    with suppress(subprocess.TimeoutExpired):
        _ = process.wait(timeout=2)
    # The session leader may exit while a descendant still ignores SIGTERM.
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        return


def phase(root: Path, work: Path, name: str, argv: list[str], bounds: Bounds) -> int:
    directory = work / name
    directory.mkdir(parents=True)
    _ = (directory / "invocation.json").write_text(
        json.dumps(
            {
                "argv": argv,
                "cwd": str(root),
                "floor": bounds.floor,
                "reserve": bounds.reserve,
                "cap": bounds.cap,
                "deadline_seconds": bounds.seconds,
                "replay_parents": [str(parent) for parent in bounds.replay_parents],
            },
            indent=2,
        )
        + "\n"
    )
    free = shutil.disk_usage(work).free
    if free < bounds.floor + bounds.reserve:
        _ = (directory / "refused.json").write_text(
            json.dumps(
                {
                    "reason": "launch reserve",
                    "actual_free": free,
                    "required": bounds.floor + bounds.reserve,
                }
            )
            + "\n"
        )
        raise CaptureError("launch reserve insufficient")
    start = time.monotonic()
    classification = "process_exit"
    diagnostic: str | None = None
    measured = bounds.measure or work
    target = measured / "target"
    transient = target if os.environ.get("CARGO_TARGET_DIR") == str(target) else None
    with (
        (directory / "stdout.log").open("wb") as stdout,
        (directory / "stderr.log").open("wb") as stderr,
        (directory / "capacity.jsonl").open("w") as samples,
        subprocess.Popen(
            argv, cwd=root, stdout=stdout, stderr=stderr, start_new_session=True
        ) as process,
    ):
        while process.poll() is None:
            elapsed = time.monotonic() - start
            free = shutil.disk_usage(work).free
            try:
                used = size(
                    measured, transient=transient, replay_parents=bounds.replay_parents
                )
            except (CaptureError, OSError) as error:
                used = -1
                classification = "inspection_error"
                diagnostic = str(error)
            _ = samples.write(
                json.dumps({"seconds": elapsed, "free": free, "logical_bytes": used})
                + "\n"
            )
            samples.flush()
            if elapsed >= bounds.seconds:
                classification = "deadline"
            if free < bounds.floor or used >= bounds.cap:
                classification = "storage_guard"
            if classification != "process_exit":
                stop_owned(process)
                break
            time.sleep(0.1)
        status = process.wait()
    try:
        if (
            size(measured, transient=transient, replay_parents=bounds.replay_parents)
            >= bounds.cap
            or shutil.disk_usage(work).free < bounds.floor
        ):
            classification = "storage_guard"
    except (CaptureError, OSError) as error:
        classification = "inspection_error"
        diagnostic = str(error)
    if time.monotonic() - start >= bounds.seconds and classification == "process_exit":
        classification = "deadline"
    _ = (directory / "result.json").write_text(
        json.dumps(
            {
                "returncode": status,
                "classification": classification,
                "monitor_error": diagnostic,
                "elapsed_seconds": time.monotonic() - start,
                "free_after": shutil.disk_usage(work).free,
            },
            indent=2,
        )
        + "\n"
    )
    return status if classification == "process_exit" else 124


def pack(source: Path, archive: Path) -> None:
    if archive.is_relative_to(source) or archive.exists():
        raise CaptureError("fresh archive outside source required")
    _ = size(source)
    archive.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(archive, "w:gz") as output:
        output.add(source, arcname=source.name, recursive=True)


def archive_bound(source: Path) -> int:
    """Reserve uncompressed payload, conservative tar headers and gzip expansion."""
    raw = size(source) + 4096 * (1 + sum(1 for _ in source.rglob("*")))
    return raw + raw // 100 + 1024**2


def existing_work(root: Path, work: Path) -> None:
    if not work.is_absolute() or work.is_relative_to(root) or work.resolve() != work:
        raise CaptureError("external job directory required")


def replay_parents(output: Path) -> tuple[Path, ...]:
    """Exact replay output parents used by matrix and refusal capture producers."""
    return (
        output,
        *(
            output / (case.name.replace("-", "_") + "_refuse_" + mutation)
            for case in CASES
            for mutation in MUTATIONS
        ),
    )


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
