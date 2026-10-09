# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 scripts/check-grf-safety-ci.py
from __future__ import annotations

import os
import sys
import tempfile
from pathlib import Path
from typing import Final

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.context_ci_support import (
    Selection,
    build_lib,
    process,
    retain_native,
    run_exact,
    sources,
    verify_identity,
    zero,
)
from scripts.depot_build_archive import package_raw
from scripts.gameplay_foundations import log_name
from scripts.grf_control_run import ControlRun
from scripts.safety_ci_bindings import bind_safety
from scripts.safety_ci_evidence import live_probes, validate_safety
from scripts.world_check_support import WorldCheckError, read_json

MATRIX: Final = "content::grf::safety_native::compare_static_safety_with_original"
GUARDS: Final = "content::grf::safety_guards::native_safety_host_guards"


def main() -> None:
    forbidden = {
        "OTTD_GRF_SAFETY_CASE",
        "OTTD_GRF_CONTROL_CASE",
        "OTTD_REPLAY_PATH",
        "OTTD_GRF_SAFETY_INPUT",
        "OTTD_GRF_SAFETY_OUTPUT",
    }
    if forbidden.intersection(os.environ):
        raise WorldCheckError(
            "Safety CI refuses partial or replay/observer environment"
        )
    root = Path(__file__).resolve().parents[1]
    layout = read_json(root / "scripts/safety-ci-layout.json")
    sources(root, layout)
    parent = root / ".artifacts"
    parent.mkdir(exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="grf-safety-", dir=parent))
    print(f"Artifacts: {output}", flush=True)
    oracle = Path(
        os.environ.get(
            "OTTD_GRF_ORACLE", str(root / ".reference/snapshot-build/openttd")
        )
    ).resolve(strict=True)
    job = ControlRun(root, output, oracle)
    job.provenance()
    binary = build_lib(job)
    variables = {
        "OTTD_GRF_ORACLE": str(oracle),
        "OTTD_GRF_SAFETY_ARTIFACTS": str(output / "results"),
        "OTTD_GRF_SAFETY_GUARDS": str(output / "guards"),
    }
    run_exact(
        job,
        binary,
        Selection(MATRIX, actual=os.environ.get("OTTD_SAFETY_TEST_SELECTOR")),
        variables,
    )
    run_exact(job, binary, Selection(GUARDS), variables)
    for test in [
        "host_bounds_accept_exact_work_and_refuse_one_less",
        "decision_budget_rejects_before_allocating_beyond_bound",
    ]:
        run_exact(
            job,
            binary,
            Selection(f"content::grf::safety_tests::{test}", ignored=False),
            {},
        )
    validate_safety(output, layout)
    bind_safety(root, output, oracle, layout)
    live_probes(output, layout)
    zero(job, binary)
    verify_identity(root, output)
    for name in [MATRIX, GUARDS]:
        process(output / "logs" / log_name(name), binary, name)
    retain_native(output, oracle)
    package_raw(output)
    _ = (output / "summary.txt").write_text(
        "PASS safety893 guards10 native6; full comparison and archive\n"
    )
    print("PASS static safety CI admission", flush=True)


if __name__ == "__main__":
    main()
