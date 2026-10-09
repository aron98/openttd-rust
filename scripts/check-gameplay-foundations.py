from __future__ import annotations

import os
import sys
from pathlib import Path

if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.depot_evidence import require_depot_evidence
from scripts.gameplay_foundations import FoundationRun, fresh_directory


def main() -> None:
    root = Path(__file__).resolve().parents[1]
    output = fresh_directory(root, os.environ.get("OTTD_FOUNDATIONS_ARTIFACTS"))
    print(f"Artifacts: {output}", flush=True)
    oracle = Path(
        os.environ.get(
            "OTTD_FOUNDATIONS_ORACLE", str(root / ".reference/snapshot-build/openttd")
        )
    ).resolve(strict=True)
    run = FoundationRun(root, output, oracle)
    run.provenance()
    binaries = run.build()
    run.test(
        binaries["native_content"],
        "native_catalog_and_price_matrix",
        {
            "OTTD_CONTENT_NATIVE_DIR": str(output / "content"),
            "OTTD_CONTENT_ORACLE": str(oracle),
        },
    )
    _ = run.run(
        "terrain",
        ["bash", "scripts/check-terrain.sh"],
        {
            "OTTD_TERRAIN_ARTIFACTS": str(output / "terrain"),
            "OTTD_TERRAIN_ORACLE": str(oracle),
        },
    )
    _ = run.run(
        "allocation-native",
        [
            "cmake",
            f"-DORACLE={oracle}",
            f"-DRUN_DIR={output / 'pools/native'}",
            f"-DCONFIG={root / 'scripts/reference.cfg'}",
            f"-DINPUT={root / 'fixtures/replay/clear-v362.sav'}",
            "-P",
            "scripts/run-allocation-reference.cmake",
        ],
    )
    run.test(
        binaries["native_runtime_pools"],
        "original_pool_and_unit_allocation_trace_matches",
        {
            "OTTD_ALLOCATION_JSON": str(output / "pools/native/allocation.json"),
            "OTTD_ALLOCATION_EVIDENCE": str(output / "pools/rust"),
        },
    )
    run.test(
        binaries["native_runtime_road"],
        "road_cache_native_matrix",
        {
            "OTTD_RUNTIME_NATIVE_DIR": str(output / "road"),
            "OTTD_RUNTIME_ORACLE": str(oracle),
        },
    )
    run.test(
        binaries["native_grf_scan"],
        "native_file_scan_matrix",
        {"OTTD_GRF_NATIVE_DIR": str(output / "grf"), "OTTD_GRF_ORACLE": str(oracle)},
    )
    run.test(
        binaries["native_depot_runtime"],
        "original_depot_runtime_matrix",
        {
            "OTTD_DEPOT_EVIDENCE": str(output / "depot"),
            "OTTD_DEPOT_ORACLE": str(oracle),
        },
    )
    require_depot_evidence(output / "depot")
    run.finish()
    print("PASS gameplay foundations", flush=True)


if __name__ == "__main__":
    main()
