# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: OTTD_DEPOT_OCCUPANCY_ORACLE=/absolute/openttd python3 scripts/check-depot-occupancy.py
from __future__ import annotations

import os
import shutil
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.depot_build_provenance import build, snapshot
from scripts.gameplay_foundations import FoundationRun, digest
from scripts.replay_matrix import ReplayMatrix
from scripts.world_check_support import (
    ROOT,
    Json,
    WorldCheckError,
    at,
    read_json,
    replace,
    run,
    write_json,
)

TEST = "commands::road_depot::occupancy::native::native_ground_cutoff"


def original(
    job: FoundationRun,
    source: Path,
    output: Path,
    *,
    prepare: bool = False,
    vectors: bool = False,
) -> None:
    variables = {"OTTD_DEPOT_OCCUPANCY_VECTORS": "1"} if vectors else {}
    _ = job.run(
        str(output.relative_to(job.output)),
        [
            "cmake",
            f"-DORACLE={job.oracle}",
            f"-DRUN_DIR={output}",
            f"-DINPUT={source}",
            f"-DCONFIG={ROOT / 'scripts/reference.cfg'}",
            f"-DPREPARE={'ON' if prepare else 'OFF'}",
            "-DVECTORS=OFF",
            "-P",
            str(ROOT / "scripts/run-depot-runtime-reference.cmake"),
        ],
        variables,
    )


def main() -> None:
    oracle = Path(os.environ["OTTD_DEPOT_OCCUPANCY_ORACLE"]).resolve(strict=True)
    _ = os.environ.pop("OTTD_DEPOT_OCCUPANCY_VECTORS", None)
    parent = ROOT / ".artifacts"
    parent.mkdir(exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="depot-occupancy-", dir=parent))
    print(f"Artifacts: {output}", flush=True)
    job = FoundationRun(ROOT, output, oracle)
    snapshot(ROOT, output, oracle)
    binaries = build(job)
    original(
        job, ROOT / "fixtures/replay/clear-v362.sav", output / "prepare", prepare=True
    )
    source = output / "prepare/save/autosave/exit.sav"
    job.test(
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
    if (
        at(
            read_json(fleet / "results.json"),
            ("actions", 0, "receipt", "exec", "success"),
        )
        is not True
    ):
        raise WorldCheckError("Original road fleet construction failed")
    job.test(
        binaries.prep,
        "prepare_depot_inputs",
        {
            "DEPOT_SOURCE": str(source),
            "DEPOT_FLEET": str(fleet / "final.sav"),
            "DEPOT_INPUTS": str(output / "inputs"),
        },
    )
    for name, saved in (
        ("flat", fleet / "final.sav"),
        ("sloped", output / "inputs/occupied-max-corner.sav"),
    ):
        case = output / name
        original(job, saved, case / "canonical")
        original(job, saved, case / "vectors", vectors=True)
        original(job, case / "vectors/save/autosave/exit.sav", case / "reload")
        case_job = FoundationRun(ROOT, case, oracle)
        case_job.test(binaries.runtime, TEST, {"DEPOT_OCCUPANCY_CASE": str(case)})
    changed = output / "corruption"
    _ = shutil.copytree(output / "sloped", changed)
    (changed / "rust-vectors.json").unlink()
    observation = changed / "vectors/depot-runtime.json"
    value = read_json(observation)
    replace(value, ("occupancy", "vectors", 0, "success"), True)
    write_json(observation, value)
    rejected = run(
        [
            "env",
            f"DEPOT_OCCUPANCY_CASE={changed}",
            str(binaries.runtime),
            "--ignored",
            "--exact",
            TEST,
        ],
        output / "rejection",
        expected=101,
    )
    if (
        "pure occupancy result differs from original" not in rejected.stdout
        or "1 failed; 0 ignored" not in rejected.stdout
    ):
        raise WorldCheckError(
            "Native occupancy mutation did not reach the actual pure comparison"
        )
    if (changed / "rust-vectors.json").exists():
        raise WorldCheckError("Rejected occupancy comparison published a result")
    provenance = read_json(output / "provenance.json")
    sources = at(provenance, ("sources",))
    match sources:
        case dict():
            for name, sha in sources.items():
                if digest(ROOT / name) != sha:
                    raise WorldCheckError("Occupancy source changed during proof")
        case _:
            raise WorldCheckError("Missing occupancy source identities")
    if digest(oracle) != at(provenance, ("oracle_sha256",)):
        raise WorldCheckError("Occupancy oracle changed during proof")
    identities = read_json(output / "test-binaries.json")
    for name in ("ottd_sim", "native_depot_build", "ottd"):
        if digest(output / "bin" / name) != at(identities, (name, "sha256")):
            raise WorldCheckError("Retained occupancy executable changed")
    files: dict[str, Json] = {
        str(path.relative_to(output)): digest(path)
        for path in sorted(output.rglob("*"))
        if path.is_file()
    }
    write_json(output / "manifest.json", files)
    write_json(
        output / "summary.json",
        {
            "passed": True,
            "loaded_road_sources": ["flat", "sloped"],
            "vectors": 4,
            "corruption_rejections": 1,
            "pure_helper_only": True,
        },
    )
    print(
        "PASS original road occupancy cutoff: 4 vectors, complete canonical/live restoration, 1 rejected mutation"
    )


if __name__ == "__main__":
    main()
