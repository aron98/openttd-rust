# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 scripts/check-grf-context-ci.py
from __future__ import annotations

import os
import shutil
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.context_ci_evidence import validate_context
from scripts.context_ci_support import (
    CONTEXT_TEST,
    GUARD_TEST,
    Selection,
    build_lib,
    native_bindings,
    process,
    refuse_subset,
    retain_native,
    run_exact,
    sources,
    verify_identity,
    zero,
)
from scripts.depot_build_archive import package_raw
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import sequence, text
from scripts.grf_control_run import ControlRun
from scripts.world_check_support import WorldCheckError, at, read_json


def main() -> None:
    refuse_subset()
    root = Path(__file__).resolve().parents[1]
    layout = read_json(root / "scripts/context-ci-layout.json")
    sources(root, layout)
    parent = root / ".artifacts"
    parent.mkdir(exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="grf-context-", dir=parent))
    print(f"Artifacts: {output}", flush=True)
    oracle = Path(
        os.environ.get(
            "OTTD_GRF_CONTEXT_ORACLE", str(root / ".reference/snapshot-build/openttd")
        )
    ).resolve(strict=True)
    job = ControlRun(root, output, oracle)
    job.provenance()
    binary = build_lib(job)
    variables = {
        "OTTD_GRF_CONTEXT_ORACLE": str(oracle),
        "OTTD_GRF_CONTEXT_ARTIFACTS": str(output / "results"),
        "OTTD_GRF_CONTEXT_GUARDS": str(output / "guards"),
    }
    run_exact(
        job,
        binary,
        Selection(CONTEXT_TEST, actual=os.environ.get("OTTD_CONTEXT_TEST_SELECTOR")),
        variables,
    )
    run_exact(job, binary, Selection(GUARD_TEST), variables)
    budgets = (
        "empty_context_load_charges_source_budget",
        "final_context_payload_obeys_exact_host_boundary",
        "offline_context_refuses_out_of_range_starting_year",
        "context_refuses_out_of_range_saved_economy_date_before_normalization",
        "context_admits_both_economy_upper_boundaries",
    )
    for name in budgets:
        run_exact(
            job,
            binary,
            Selection(f"content::grf::load_context_tests::{name}", ignored=False),
            {},
        )
    validate_context(output, layout)
    zero(job, binary)
    verify_identity(root, output)
    for name in (CONTEXT_TEST, GUARD_TEST):
        process(output / "logs" / name, binary, name)
    if digest(output / "results/context-test-binary") != digest(binary):
        raise WorldCheckError("Context actual executable differs from Cargo selection")
    baseline = output / "baselines"
    baseline.mkdir()
    raw = read_json(output / "results/globals-temperate/native/control.json")
    for index, path in enumerate(sequence(at(raw, ("baseline_sources",)))):
        _ = shutil.copy2(text(path), baseline / f"{index}.grf")
    retain_native(output, oracle)
    native_bindings(root, output, oracle)
    package_raw(output)
    _ = (output / "summary.txt").write_text(
        "PASS context66 guards18 mutations2046 exact budgets and complete archive\n"
    )
    print("PASS private GRF context CI admission", flush=True)


if __name__ == "__main__":
    main()
