from __future__ import annotations

import shutil
from pathlib import Path

from scripts.gameplay_foundations import digest
from scripts.order_state_provenance import validate_native_guard
from scripts.owned_restore_run import RestoreRun
from scripts.world_check_support import WorldCheckError, run, write_json


def relocated_stamp(stamp: str, original: Path, copied: Path, sha: str) -> str:
    first = f"{sha}  {original}\n"
    if not stamp.startswith(first) or stamp.count(str(original)) != 1:
        raise WorldCheckError("Native source stamp executable identity differs")
    return f"{sha}  {copied}\n" + stamp[len(first) :]


def freshness_control(job: RestoreRun) -> None:
    directory = job.output / "controls/native"
    directory.mkdir(parents=True, exist_ok=True)
    binary = directory / "openttd"
    _ = shutil.copy2(job.oracle, binary)
    stamp = relocated_stamp(
        (job.oracle.parent / "replay-build.sha256").read_text(),
        job.oracle,
        binary,
        digest(job.oracle),
    )
    _ = (directory / "replay-build.sha256").write_text(stamp)
    argv = [
        "cmake",
        f"-DORACLE={binary}",
        "-P",
        str(job.root / "scripts/check-replay-build.cmake"),
    ]
    _ = run(argv, directory / "before")
    before = binary.read_bytes()
    before_hash = digest(binary)
    binary.chmod(0o755)
    _ = binary.write_bytes(before[:-1] + bytes([before[-1] ^ 1]))
    write_json(
        directory / "mutation.json",
        {
            "before_sha256": before_hash,
            "after_sha256": digest(binary),
            "byte_offset": len(before) - 1,
            "xor": 1,
        },
    )
    _ = run(argv, directory / "rejected", expected=1)
    validate_native_guard(job.root, job.output, job.oracle)
    if (directory / "replay-build.sha256").read_text() != stamp:
        raise WorldCheckError("Native relocated freshness stamp changed")
