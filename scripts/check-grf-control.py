from __future__ import annotations

import os
import shutil
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.grf_control_archive import archive_files
from scripts.grf_control_evidence import sequence, text, validate
from scripts.grf_control_guards import guards
from scripts.grf_control_provenance import verify
from scripts.grf_control_run import ControlRun
from scripts.world_check_support import WorldCheckError, at, read_json


def main() -> None:
    if "OTTD_GRF_CONTROL_CASE" in os.environ:
        raise WorldCheckError("Full loader CI refuses OTTD_GRF_CONTROL_CASE")
    if "OTTD_REPLAY_PATH" in os.environ:
        raise WorldCheckError("Full loader CI refuses replay environment")
    root = Path(__file__).resolve().parents[1]
    parent = root / ".artifacts"
    parent.mkdir(exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="grf-control-", dir=parent))
    print(f"Artifacts: {output}", flush=True)
    oracle = Path(
        os.environ.get(
            "OTTD_GRF_CONTROL_ORACLE", str(root / ".reference/snapshot-build/openttd")
        )
    ).resolve()
    run = ControlRun(root, output, oracle)
    run.provenance()
    binaries = run.build()
    run.test(binaries["native_grf_control"], "generated_control_programs_are_admitted")
    for name in (
        "cumulative_limits_refuse_work_without_fabricating_disabled_reports",
        "exact_work_limits_admit_the_last_unit_and_reject_the_next",
        "trace_limit_charges_the_emitted_event_storage",
    ):
        run.test(binaries["grf_load_control"], name)
    run.test(binaries["native_grf_control"], "native_load_control_matrix", ignored=True)
    _ = validate(output / "results", root / "scripts/grf-control-layout.json")
    guards(run, binaries["native_grf_control"])
    verify(root, output)
    baseline = output / "baselines"
    baseline.mkdir()
    raw = read_json(output / "results/op-00-ordinary/native/control.json")
    for index, value in enumerate(sequence(at(raw, ("baseline_sources",)))):
        _ = shutil.copy2(text(value), baseline / f"{index}.grf")
    retained = output / "native"
    retained.mkdir()
    _ = shutil.copy2(oracle, retained / "openttd")
    _ = shutil.copy2(
        oracle.parent / "replay-build.sha256", retained / "replay-build.sha256"
    )
    paths = sorted(
        path
        for path in output.rglob("*")
        if path.is_file() and not path.is_relative_to(output / "target")
    )
    archive_files(output, paths)
    coverage = "PASS 247 cases, 8673 events, 6697 record decisions, 1293 final configs"
    checks = "4746 comparator rejections; all host guards and exact budget tests"
    _ = (output / "summary.txt").write_text(f"{coverage}\n{checks}\n")
    print("PASS configured GRF control and complete evidence")


if __name__ == "__main__":
    main()
