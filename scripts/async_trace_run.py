"""Run one unchanged native replay and retain exact process/input identities."""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path

from scripts.async_trace_sources import verify
from scripts.movement_ci.value import require


@dataclass(frozen=True, slots=True)
class Replay:
    binary: Path
    save: Path
    actions: Path
    config: Path
    directory: Path
    trace: bool
    routes: bool


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(replay: Replay) -> int:
    verify(Path(__file__).resolve().parents[1])
    directory = replay.directory.resolve()
    directory.mkdir(parents=True, exist_ok=False)
    for source, name in (
        (replay.actions, "actions.json"),
        (replay.config, "openttd.cfg"),
    ):
        _ = shutil.copy2(source, directory / name)
    environment = {
        key: value
        for key, value in os.environ.items()
        if key in {"HOME", "PATH", "TMPDIR", "LANG", "LC_ALL", "TZ"}
    }
    environment.update(
        OTTD_REPLAY_PATH=str(directory / "actions.json"),
        OTTD_REPLAY_OUTPUT=str(directory),
        OTTD_FULL_RUNTIME_OBSERVE="1",
    )
    if replay.trace:
        environment["OTTD_ASYNC_TRACE"] = str(directory / "async.jsonl")
    if replay.routes:
        environment["OTTD_ROAD_ROUTE_OBSERVE"] = "1"
    argv = [
        str(replay.binary.resolve()),
        "-X",
        "-x",
        "-c",
        str(directory / "openttd.cfg"),
        "-snull",
        "-mnull",
        "-vnull:ticks=8",
        "-g",
        str(replay.save.resolve()),
    ]
    for name, value in (
        ("argv", argv),
        ("environment", environment),
        (
            "bindings",
            {
                "binary": str(replay.binary.resolve()),
                "binary_sha256": digest(replay.binary),
                "save": str(replay.save.resolve()),
                "save_sha256": digest(replay.save),
                "actions_sha256": digest(replay.actions),
                "config_sha256": digest(replay.config),
            },
        ),
    ):
        _ = (directory / f"{name}.json").write_text(json.dumps(value, indent=2) + "\n")
    started = time.monotonic()
    with (
        (directory / "stdout.log").open("wb") as stdout,
        (directory / "stderr.log").open("wb") as stderr,
    ):
        process = subprocess.run(
            argv,
            cwd=directory,
            env=environment,
            stdout=stdout,
            stderr=stderr,
            timeout=240,
            check=False,
        )
    _ = (directory / "process.json").write_text(
        json.dumps(
            {
                "returncode": process.returncode,
                "elapsed_seconds": time.monotonic() - started,
            }
        )
        + "\n",
    )
    return process.returncode


def main() -> int:
    require(
        "binary save actions config output trace(on/off) routes(on/off)",
        condition=len(sys.argv) == 8,
    )
    require(
        "invalid switch",
        condition=sys.argv[6] in {"on", "off"} and sys.argv[7] in {"on", "off"},
    )
    return run(
        Replay(
            Path(sys.argv[1]),
            Path(sys.argv[2]),
            Path(sys.argv[3]),
            Path(sys.argv[4]),
            Path(sys.argv[5]),
            sys.argv[6] == "on",
            sys.argv[7] == "on",
        )
    )


if __name__ == "__main__":
    raise SystemExit(main())
