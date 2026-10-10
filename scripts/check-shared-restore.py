# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
from __future__ import annotations

import os
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.backup_enabled_sale_provenance import build
from scripts.backup_sale_run import member
from scripts.context_ci_support import native_bindings, retain_native
from scripts.depot_build_archive import bounded_paths, package_raw
from scripts.grf_control_evidence import sequence, text
from scripts.grf_control_run import ControlRun
from scripts.owned_restore_freshness import freshness_control
from scripts.owned_restore_run import MALLOC, RestoreRun
from scripts.shared_restore_admission import admission_controls
from scripts.shared_restore_archive import archive_controls
from scripts.shared_restore_controls import semantic_controls
from scripts.shared_restore_evidence import validate
from scripts.shared_restore_guards import observer_controls
from scripts.shared_restore_pair import pair
from scripts.shared_restore_prepare import prepare
from scripts.shared_restore_provenance import snapshot, verify_all
from scripts.shared_restore_units import units
from scripts.world_check_support import WorldCheckError, at, read_json, write_json


def main() -> None:
    if len(sys.argv) != 1 or any(
        (key.startswith("OTTD_") and key != "OTTD_SHARED_RESTORE_ORACLE")
        or key.startswith(("OWNED_RESTORE_", "SHARED_RESTORE_", "CARGO_PROFILE_"))
        or key in MALLOC
        or key
        in {
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
            "RUSTC",
            "RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER",
            "CARGO_BUILD_TARGET",
            "CARGO_BUILD_RUSTFLAGS",
        }
        for key in os.environ
    ):
        raise WorldCheckError(
            "Complete Restore CI refuses subset or inherited case environment"
        )
    root = Path(__file__).resolve().parents[1]
    oracle = Path(
        os.environ.get(
            "OTTD_SHARED_RESTORE_ORACLE",
            str(root / ".reference/snapshot-build/openttd"),
        )
    ).resolve(strict=True)
    parent = root / ".artifacts"
    parent.mkdir(exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="shared-restore-", dir=parent))
    print(f"Artifacts: {output}", flush=True)
    layout = read_json(root / "scripts/shared-restore-layout.json")
    snapshot(root, output, layout)
    control = ControlRun(root, output, oracle)
    _ = control.run(
        "freshness",
        [
            "cmake",
            f"-DORACLE={oracle}",
            "-P",
            str(root / "scripts/check-replay-build.cmake"),
        ],
    )
    control.provenance()
    _ = control.run("rust-version", ["rustc", "--version", "--verbose"])
    _ = control.run("python-version", ["python3", "--version"])
    target = Path(tempfile.mkdtemp(prefix="shared-restore-target-", dir=parent))
    binaries = build(control, target)
    job = RestoreRun(
        root,
        output,
        oracle,
        binaries,
        read_json(root / "scripts/shared-restore-fixtures.json"),
    )
    units(job)
    prepare(job)
    for entry in sequence(at(job.fixtures, ("cases",))):
        name = text(at(entry, ("name",)))
        job.native(
            member(output / "results", text(at(entry, ("source",)))),
            member(output / "results/cases", name),
            at(entry, ("descriptor",)),
        )
        pair(job, name)
    coverage = validate(job)
    semantic_controls(job)
    observer_controls(job)
    freshness_control(job)
    admission_controls(job, layout)
    archive_controls(job)
    verify_all(job, layout)
    retain_native(output, oracle)
    native_bindings(root, output, oracle)
    write_json(output / "coverage.json", coverage)
    expected = {text(value) for value in sequence(at(layout, ("base_paths",)))}
    match at(layout, ("sources",)):
        case dict() as sources:
            expected.update("source/" + name for name in sources)
        case _:
            raise WorldCheckError("Missing Restore source archive members")
    for key, prefix in (("result_paths", "results/"), ("control_paths", "controls/")):
        expected.update(prefix + text(value) for value in sequence(at(layout, (key,))))
    _ = bounded_paths(output, expected)
    package_raw(output)
    _ = (output / "summary.txt").write_text(
        "PASS shared/default Restore CI admission\n"
    )
    print("PASS shared/default Restore CI admission", flush=True)


if __name__ == "__main__":
    main()
