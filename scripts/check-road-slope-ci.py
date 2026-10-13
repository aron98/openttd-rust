# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 scripts/check-road-slope-ci.py
from __future__ import annotations

import os
import shutil
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.context_ci_support import (
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
from scripts.road_slope_ci_evidence import SLOPE_TEST, validate_slope
from scripts.world_check_support import WorldCheckError, at, read_json, write_json


def main() -> None:
    refuse_subset()
    if {
        "ROAD_SLOPE_CASE",
        "OTTD_ROAD_SLOPE_PATH",
        "OTTD_ROAD_SLOPE_CASE",
        "OTTD_DEPOT_OCCUPANCY_VECTORS",
    }.intersection(os.environ):
        raise WorldCheckError(
            "Complete road slope CI refuses subset or inherited observation environment"
        )
    root = Path(__file__).resolve().parents[1]
    layout = read_json(root / "scripts/road-slope-ci-layout.json")
    sources(root, layout)
    oracle = Path(
        os.environ.get(
            "OTTD_ROAD_SLOPE_ORACLE",
            str(root / ".reference/snapshot-build/openttd"),
        )
    ).resolve(strict=True)
    parent = root / ".artifacts"
    parent.mkdir(exist_ok=True)
    previous = set(parent.iterdir())
    staging = Path(tempfile.mkdtemp(prefix="road-slope-invocation-", dir=parent))
    invocation = ControlRun(root, staging, oracle)
    result = invocation.run(
        "driver",
        [sys.executable, "scripts/check-road-slope.py"],
        {"OTTD_ROAD_SLOPE_ORACLE": str(oracle), "CARGO_INCREMENTAL": "0"},
    )
    announcements = [
        line.removeprefix("Artifacts: ")
        for line in result.stdout.splitlines()
        if line.startswith("Artifacts: ")
    ]
    if len(announcements) != 1:
        raise WorldCheckError("Expected one actual slope artifact directory")
    output = Path(announcements[0]).resolve(strict=True)
    if (
        output in previous
        or output.parent != parent
        or not output.name.startswith("road-slope-")
    ):
        raise WorldCheckError("Occupancy driver escaped artifact root")
    _ = shutil.move(str(staging / "logs/driver"), output / "ci-invocation")
    (staging / "logs").rmdir()
    staging.rmdir()
    print(f"Artifacts: {output}", flush=True)
    validate_slope(root, output, layout)
    for name in ("ottd_sim", "native_depot_build", "ottd"):
        (output / "bin" / name).chmod(0o555)
    identity = at(read_json(output / "test-binaries.json"), ("ottd_sim", "retained"))
    binary = Path(text(identity))
    job = ControlRun(root, output, oracle)
    run_exact(
        job,
        binary,
        Selection(SLOPE_TEST, actual=os.environ.get("OTTD_ROAD_SLOPE_TEST_SELECTOR")),
        {"ROAD_SLOPE_CASE": str(output / "case")},
    )
    coverage = read_json(output / "coverage.json")
    match coverage:
        case dict():
            coverage["ci_confirmation_invocations"] = 1
        case _:
            raise WorldCheckError("Missing slope coverage")
    write_json(output / "coverage.json", coverage)
    zero(job, binary)
    verify_identity(root, output, occupancy=True)
    retain_native(output, oracle)
    native_bindings(root, output, oracle)
    match read_json(output / "native-bindings.json"):
        case dict() as bindings if set(bindings) == {
            "prepare/invocation.txt",
            "original-fleet/native/invocation.txt",
            "case/canonical/invocation.txt",
            "case/vectors/invocation.txt",
            "case/reload/invocation.txt",
        }:
            pass
        case _:
            raise WorldCheckError("Original slope invocation identities differ")
    package_raw(output)
    _ = (output / "summary.txt").write_text(
        "PASS road-slope155648+5 original mutation and complete restoration/archive\n"
    )
    print("PASS pure slope CI admission", flush=True)


if __name__ == "__main__":
    main()
