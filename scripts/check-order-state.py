# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 scripts/check-order-state.py
from __future__ import annotations

import os
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.context_ci_support import native_bindings, retain_native
from scripts.depot_build_archive import package_raw
from scripts.grf_control_run import ControlRun
from scripts.order_state_admission import validate
from scripts.order_state_controls import (
    capture_controls,
    execution_controls,
    native_admission,
    semantic_controls,
)
from scripts.order_state_fixtures import (
    OrderRun,
    airport_geometry,
    hangar,
    orphan,
    single_player,
)
from scripts.order_state_network import network, roles_and_boundaries
from scripts.order_state_provenance import build, snapshot, verify
from scripts.world_check_support import WorldCheckError, read_json, write_json


def main() -> None:
    if any(
        (key.startswith("OTTD_") and key != "OTTD_ORDER_ORACLE")
        or key.startswith(("ORDER_CASE_", "ORDER_AIRPORT_"))
        for key in os.environ
    ):
        raise WorldCheckError(
            "Complete order CI refuses subset or inherited case environment"
        )
    root = Path(__file__).resolve().parents[1]
    oracle = Path(
        os.environ.get(
            "OTTD_ORDER_ORACLE", str(root / ".reference/snapshot-build/openttd")
        )
    ).resolve(strict=True)
    parent = root / ".artifacts"
    parent.mkdir(exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="order-state-", dir=parent))
    print(f"Artifacts: {output}", flush=True)
    layout = read_json(root / "scripts/order-state-layout.json")
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
    target = Path(tempfile.mkdtemp(prefix="order-target-", dir=parent))
    manifest = build(control, target)
    job = OrderRun(
        root,
        output,
        oracle,
        manifest,
        read_json(root / "scripts/order-state-fixtures.json"),
    )
    single_player(job)
    orphan(job)
    hangar(job)
    airport_geometry(job)
    network(job, "network/single")
    network(job, "network/multi")
    roles_and_boundaries(job)
    network(job, "capture/reused", mode="reused")
    network(job, "capture/unarmed", mode="unarmed")
    execution_controls(job)
    semantic_controls(job)
    capture_controls(job)
    native_admission(job)
    verify(root, output, layout, manifest)
    retain_native(output, oracle)
    native_bindings(root, output, oracle)

    coverage = validate(job, layout)
    write_json(output / "coverage.json", coverage)
    package_raw(output)
    _ = (output / "summary.txt").write_text("PASS order-state CI admission\n")
    print("PASS order-state CI admission", flush=True)


if __name__ == "__main__":
    main()
