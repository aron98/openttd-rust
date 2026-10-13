# SPDX-License-Identifier: GPL-2.0-only
"""Execute the complete-loop Rust forest oracle with immutable binary/input receipts."""

from pathlib import Path

from scripts.gameplay_foundations import digest, require_test
from scripts.grf_control_run import ControlRun
from scripts.world_check_support import write_json

ROOT = Path(__file__).resolve().parents[1]
TEST = "native_industry_lifecycle_matches_complete_runtime"


def run(binary: Path, native: Path, output: Path) -> None:
    result = ControlRun(ROOT, output.with_name(output.name + "-logs"), binary).run(
        "lifecycle",
        [str(binary), "--ignored", "--exact", TEST, "--nocapture"],
        {
            "OTTD_INDUSTRY_NATIVE_DIR": str(native),
            "OTTD_INDUSTRY_RUST_OUTPUT": str(output),
        },
    )
    require_test(result.stdout, TEST)
    write_json(
        output / "bindings.json",
        {
            "binary": str(binary),
            "binary_sha256": digest(binary),
            "native": str(native),
            "input_sha256": digest(native / "initial.sav"),
            "protocol_sha256": digest(native / "protocol.json"),
        },
    )
