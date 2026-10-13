"""Measure owned capture storage while coordinating its retained archive rotation."""

from __future__ import annotations

import fcntl
import os
import re
import stat
import time
from collections.abc import Generator
from contextlib import ExitStack, closing, contextmanager
from pathlib import Path


class CaptureError(RuntimeError):
    """A capture orchestration boundary was refused."""


@contextmanager
def archive_lock(directory: Path, *, exclusive: bool) -> Generator[None, None, None]:
    """Bound advisory coordination on the directory inode without following links."""
    descriptor = os.open(directory, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        operation = fcntl.LOCK_EX if exclusive else fcntl.LOCK_SH
        deadline = time.monotonic() + 2
        while True:
            try:
                fcntl.flock(descriptor, operation | fcntl.LOCK_NB)
                break
            except BlockingIOError:
                if time.monotonic() >= deadline:
                    raise CaptureError("archive rotation lock deadline") from None
                time.sleep(0.01)
        yield
    finally:
        os.close(descriptor)


def size(
    path: Path,
    *,
    transient: Path | None = None,
    replay_parents: tuple[Path, ...] = (),
    archive_parent: Path | None = None,
) -> int:
    """Count live bytes; only explicitly owned scratch directories may disappear."""
    for parent in (*replay_parents, *((archive_parent,) if archive_parent else ())):
        if (
            not parent.is_absolute()
            or parent.resolve() != parent
            or parent == path
            or not parent.is_relative_to(path)
        ):
            raise CaptureError("replay parent must be canonical inside measured job")
    total = 0
    with closing(_members(path, transient, replay_parents, archive_parent)) as members:
        for member, removable in members:
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
    path: Path,
    transient: Path | None,
    replay_parents: tuple[Path, ...],
    archive_parent: Path | None,
) -> Generator[tuple[Path, bool], None, None]:
    with ExitStack() as locks:
        pending = [(path, False)] if path.exists() else []
        while pending:
            directory, disposable = pending.pop()
            if directory == archive_parent:
                _ = locks.enter_context(archive_lock(directory, exclusive=False))
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
                            and re.fullmatch(
                                r"\.ottd-replay-[A-Za-z0-9]{6}", entry.name
                            )
                            is not None
                            and is_directory
                        )
                    )
                    yield member, removable
                    if is_directory:
                        pending.append((member, removable))
