"""Run one guarded native rating observation with exact input identities."""

import hashlib
import json
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

from scripts.company_rating_events import read_events
from scripts.company_rating_model import read_snapshot
from scripts.company_rating_sources import verify
from scripts.movement_ci.value import require


def main() -> None:
    verify(Path(__file__).resolve().parents[1])
    require(
        "binary save actions config output on|off",
        condition=len(sys.argv) == 7,
    )
    require(
        "invalid company-rating trace switch",
        condition=sys.argv[6] in {"on", "off"},
    )
    binary, save, actions, config, output = (
        Path(value).resolve() for value in sys.argv[1:6]
    )
    enabled = sys.argv[6] == "on"
    output.mkdir(parents=True, exist_ok=False)
    for source, name in ((actions, "actions.json"), (config, "openttd.cfg")):
        _ = shutil.copy2(source, output / name)
    environment = {
        key: value
        for key, value in os.environ.items()
        if key in {"HOME", "PATH", "TMPDIR", "LANG", "LC_ALL", "TZ"}
    }
    environment.update(
        OTTD_REPLAY_PATH=str(output / "actions.json"),
        OTTD_REPLAY_OUTPUT=str(output),
        OTTD_FULL_RUNTIME_OBSERVE="1",
        OTTD_VEHICLE_YEAR_TRACE=str(output / "year.jsonl"),
        OTTD_COMPANY_RATING_OBSERVE="1",
    )
    if enabled:
        environment["OTTD_COMPANY_RATING_TRACE"] = str(output / "rating.jsonl")
    argv = [
        str(binary),
        "-X",
        "-x",
        "-c",
        str(output / "openttd.cfg"),
        "-snull",
        "-mnull",
        "-vnull:ticks=8",
        "-g",
        str(save),
    ]
    identities = [
        {
            "path": str(path),
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "mode": path.stat().st_mode,
            "lstat_mode": path.lstat().st_mode,
        }
        for path in (binary, save, actions, config, Path(__file__).resolve())
    ]
    _ = (output / "invocation.json").write_text(
        json.dumps(
            {
                "argv": argv,
                "cwd": str(output),
                "environment": environment,
                "inputs": identities,
            },
            indent=2,
        )
        + "\n"
    )
    start = time.monotonic()
    with (
        (output / "stdout.log").open("wb") as stdout,
        (output / "stderr.log").open("wb") as stderr,
    ):
        result = subprocess.run(
            argv,
            cwd=output,
            env=environment,
            stdout=stdout,
            stderr=stderr,
            check=False,
            timeout=300,
        )
    _ = (output / "process.json").write_text(
        json.dumps(
            {
                "returncode": result.returncode,
                "elapsed_seconds": time.monotonic() - start,
            }
        )
        + "\n"
    )
    if result.returncode == 0:
        checkpoints = tuple(output.glob("*.world.json"))
        require("native checkpoint coordinates required", condition=bool(checkpoints))
        for checkpoint in checkpoints:
            label = checkpoint.name.removesuffix(".world.json")
            _ = read_snapshot(output / f"{label}.rating.json")
        trace = output / "rating.jsonl"
        if trace.exists():
            _ = read_events(trace)
    raise SystemExit(result.returncode)


if __name__ == "__main__":
    main()
