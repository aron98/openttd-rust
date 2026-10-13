"""External process bounds remain distinct from native simulation outcomes."""

from __future__ import annotations

import shutil
import subprocess
import time
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path

from .value import require


def size(directory: Path) -> int:
    total = 0
    for path in directory.rglob("*"):
        require("evidence symlink forbidden", condition=not path.is_symlink())
        if path.is_file():
            total += path.stat().st_size
    return total


@dataclass(frozen=True, slots=True)
class Limits:
    directory: Path
    proof: Path
    run_bytes: int
    proof_bytes: int
    free_floor: int
    seconds: int


@dataclass(frozen=True, slots=True)
class Process:
    code: int
    classification: str
    seconds: float
    directory_bytes: int
    proof_bytes: int


def run(argv: list[str], environment: Mapping[str, str], limits: Limits) -> Process:
    started = time.monotonic()
    classification = "native_process_exit"
    with (
        (limits.directory / "stdout.log").open("wb") as stdout,
        (limits.directory / "stderr.log").open("wb") as stderr,
        subprocess.Popen(
            argv, cwd=limits.directory, env=environment, stdout=stdout, stderr=stderr
        ) as process,
    ):
        while process.poll() is None:
            if (
                size(limits.directory) >= limits.run_bytes
                or size(limits.proof) >= limits.proof_bytes
                or shutil.disk_usage(limits.proof).free < limits.free_floor
            ):
                classification = "host_storage_budget"
                process.kill()
                break
            if time.monotonic() - started >= limits.seconds:
                classification = "host_time_budget"
                process.kill()
                break
            time.sleep(0.05)
        code = process.wait()
    if (
        size(limits.directory) >= limits.run_bytes
        or size(limits.proof) >= limits.proof_bytes
        or shutil.disk_usage(limits.proof).free < limits.free_floor
    ):
        classification = "host_storage_budget"
    if (
        time.monotonic() - started >= limits.seconds
        and classification == "native_process_exit"
    ):
        classification = "host_time_budget"
    return Process(
        code,
        classification,
        time.monotonic() - started,
        size(limits.directory),
        size(limits.proof),
    )
