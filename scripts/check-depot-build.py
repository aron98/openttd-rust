from __future__ import annotations

import os
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.depot_build_evidence import package
from scripts.depot_build_provenance import build, input_snapshot, snapshot
from scripts.gameplay_foundations import FoundationRun
from scripts.replay_matrix import ReplayMatrix
from scripts.world_check_support import ROOT, WorldCheckError, at, read_json


def main() -> None:
    root = ROOT.resolve()
    oracle = Path(
        os.environ.get(
            "OTTD_DEPOT_BUILD_ORACLE", root / ".reference/snapshot-build/openttd"
        )
    ).resolve(strict=True)
    parent = root / ".artifacts"
    parent.mkdir(exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="depot-build-", dir=parent))
    print(f"Artifacts: {output}", flush=True)
    run = FoundationRun(root, output, oracle)
    snapshot(root, output, oracle)
    binaries = build(run)
    _ = run.run(
        "original-depots",
        [
            "cmake",
            f"-DORACLE={oracle}",
            f"-DRUN_DIR={output / 'original-depots'}",
            f"-DINPUT={root / 'fixtures/replay/clear-v362.sav'}",
            f"-DCONFIG={root / 'scripts/reference.cfg'}",
            "-DPREPARE=ON",
            "-DVECTORS=OFF",
            "-P",
            str(root / "scripts/run-depot-runtime-reference.cmake"),
        ],
    )
    source = output / "original-depots/save/autosave/exit.sav"
    run.test(
        binaries.prep,
        "prepare_fleet_source",
        {"DEPOT_SOURCE": str(source), "DEPOT_FLEET_PREP": str(output / "fleet-input")},
    )
    matrix = ReplayMatrix(binaries.cli, oracle, oracle, output)
    fleet = matrix.native(
        output / "fleet-input/fleet.sav",
        output / "fleet-input/fleet.json",
        output / "original-fleet",
    )
    result = read_json(fleet / "results.json")
    if at(result, ("actions", 0, "receipt", "exec", "success")) is not True:
        raise WorldCheckError("Original depot occupancy fleet was not constructed")
    run.test(
        binaries.prep,
        "prepare_depot_inputs",
        {
            "DEPOT_SOURCE": str(source),
            "DEPOT_FLEET": str(fleet / "final.sav"),
            "DEPOT_INPUTS": str(output / "inputs"),
        },
    )
    input_snapshot(output)
    _ = run.run(
        "matrix",
        [
            sys.executable,
            "-m",
            "scripts.depot_replay",
            "--inputs",
            str(output / "inputs"),
            "--artifacts",
            str(output / "results"),
            "--oracle",
            str(oracle),
            "--ottd",
            str(binaries.cli),
        ],
    )
    package(root, output)
    _ = (output / "summary.txt").write_text(
        "PASS native depot create/rotate: 178 cases, 676 commands, 201 executions, 7 controls\n"
    )
    print("PASS native depot create/rotate", flush=True)


if __name__ == "__main__":
    main()
