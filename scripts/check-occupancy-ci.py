# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 scripts/check-occupancy-ci.py
from __future__ import annotations

import os
import shutil
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.context_ci_support import (
    OCCUPANCY_TEST,
    Selection,
    native_bindings,
    refuse_subset,
    retain_native,
    run_exact,
    sources,
    verify_identity,
    zero,
)
from scripts.depot_build_archive import package_raw
from scripts.grf_control_evidence import text
from scripts.grf_control_run import ControlRun
from scripts.occupancy_ci_evidence import validate_occupancy
from scripts.world_check_support import WorldCheckError, at, read_json, write_json


def main() -> None:
    refuse_subset()
    root = Path(__file__).resolve().parents[1]
    layout = read_json(root / "scripts/occupancy-ci-layout.json")
    sources(root, layout)
    oracle = Path(
        os.environ.get(
            "OTTD_DEPOT_OCCUPANCY_ORACLE",
            str(root / ".reference/snapshot-build/openttd"),
        )
    ).resolve(strict=True)
    parent = root / ".artifacts"
    parent.mkdir(exist_ok=True)
    staging = Path(tempfile.mkdtemp(prefix="occupancy-invocation-", dir=parent))
    invocation = ControlRun(root, staging, oracle)
    result = invocation.run(
        "driver",
        [sys.executable, "scripts/check-depot-occupancy.py"],
        {"OTTD_DEPOT_OCCUPANCY_ORACLE": str(oracle), "CARGO_INCREMENTAL": "0"},
    )
    announcements = [
        line.removeprefix("Artifacts: ")
        for line in result.stdout.splitlines()
        if line.startswith("Artifacts: ")
    ]
    if len(announcements) != 1:
        raise WorldCheckError("Expected one actual occupancy artifact directory")
    output = Path(announcements[0]).resolve(strict=True)
    if output.parent != parent or not output.name.startswith("depot-occupancy-"):
        raise WorldCheckError("Occupancy driver escaped artifact root")
    _ = shutil.move(str(staging / "logs/driver"), output / "ci-invocation")
    (staging / "logs").rmdir()
    staging.rmdir()
    print(f"Artifacts: {output}", flush=True)
    validate_occupancy(root, output, layout)
    for name in ("ottd_sim", "native_depot_build", "ottd"):
        (output / "bin" / name).chmod(0o555)
    identity = at(read_json(output / "test-binaries.json"), ("ottd_sim", "retained"))
    binary = Path(text(identity))
    job = ControlRun(root, output, oracle)
    run_exact(
        job,
        binary,
        Selection(
            OCCUPANCY_TEST, actual=os.environ.get("OTTD_OCCUPANCY_TEST_SELECTOR")
        ),
        {"DEPOT_OCCUPANCY_CASE": str(output / "flat")},
    )
    coverage = read_json(output / "coverage.json")
    match coverage:
        case dict():
            coverage["ci_confirmation_invocations"] = 1
        case _:
            raise WorldCheckError("Missing occupancy coverage")
    write_json(output / "coverage.json", coverage)
    zero(job, binary)
    verify_identity(root, output, occupancy=True)
    retain_native(output, oracle)
    native_bindings(root, output, oracle)
    package_raw(output)
    _ = (output / "summary.txt").write_text(
        "PASS occupancy4 original mutation and complete restoration/archive\n"
    )
    print("PASS pure occupancy CI admission", flush=True)


if __name__ == "__main__":
    main()
