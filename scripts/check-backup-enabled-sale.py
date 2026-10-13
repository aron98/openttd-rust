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

from scripts.backup_enabled_sale_controls import controls, validate_controls
from scripts.backup_enabled_sale_evidence import validate
from scripts.backup_enabled_sale_provenance import build, snapshot, verify_all
from scripts.backup_sale_pair import pair
from scripts.backup_sale_run import BackupSaleRun
from scripts.context_ci_support import native_bindings, retain_native
from scripts.depot_build_archive import bounded_paths, package_raw
from scripts.grf_control_evidence import sequence, text
from scripts.grf_control_run import ControlRun
from scripts.world_check_support import WorldCheckError, at, read_json, write_json


def main() -> None:
    if len(sys.argv) != 1 or any(
        (key.startswith("OTTD_") and key != "OTTD_BACKUP_ENABLED_SALE_ORACLE")
        or key.startswith("BACKUP_ENABLED_SALE_")
        for key in os.environ
    ):
        raise WorldCheckError(
            "Complete backup sale CI refuses subset or inherited case environment"
        )
    root = Path(__file__).resolve().parents[1]
    oracle = Path(
        os.environ.get(
            "OTTD_BACKUP_ENABLED_SALE_ORACLE",
            str(root / ".reference/snapshot-build/openttd"),
        )
    ).resolve(strict=True)
    parent = root / ".artifacts"
    parent.mkdir(exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="backup-enabled-sale-", dir=parent))
    print(f"Artifacts: {output}", flush=True)
    layout = read_json(root / "scripts/backup-enabled-sale-layout.json")
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
    target = Path(tempfile.mkdtemp(prefix="backup-enabled-sale-target-", dir=parent))
    binaries = build(control, target)
    job = BackupSaleRun(
        root,
        output,
        oracle,
        binaries,
        read_json(root / "scripts/backup-enabled-sale-fixtures.json"),
        backup_enabled=True,
    )
    job.prepare()
    for name in sequence(at(job.fixtures, ("cases",))):
        pair(job, text(name))
    controls(job, layout)
    validate_controls(job, layout)
    verify_all(job, layout)
    retain_native(output, oracle)
    native_bindings(root, output, oracle)
    coverage = validate(job, layout)
    _ = bounded_paths(
        output / "controls", {text(v) for v in sequence(at(layout, ("control_paths",)))}
    )
    write_json(output / "coverage.json", coverage)
    expected = {text(v) for v in sequence(at(layout, ("base_paths",)))}
    match at(layout, ("sources",)):
        case dict() as source_pins:
            expected.update("source/" + name for name in source_pins)
        case _:
            raise WorldCheckError("Missing exact source archive members")
    for key, prefix in (("result_paths", "results/"), ("control_paths", "controls/")):
        expected.update(prefix + text(v) for v in sequence(at(layout, (key,))))
    _ = bounded_paths(output, expected)
    package_raw(output)
    _ = (output / "summary.txt").write_text("PASS backup-enabled-sale CI admission\n")
    print("PASS backup-enabled-sale CI admission", flush=True)


if __name__ == "__main__":
    main()
