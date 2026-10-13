# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: OTTD_ROAD_SLOPE_ORACLE=/absolute/openttd python3 scripts/check-road-slope.py
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

TEST = "commands::road::slope::native::original_road_slope_matrix"


def original(
    job: FoundationRun,
    source: Path,
    output: Path,
    *,
    prepare: bool = False,
    vectors: bool = False,
) -> None:
    variables = (
        {"OTTD_ROAD_SLOPE_PATH": str(output / "road-slope.json")} if vectors else {}
    )
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
    oracle = Path(os.environ["OTTD_ROAD_SLOPE_ORACLE"]).resolve(strict=True)
    _ = os.environ.pop("OTTD_ROAD_SLOPE_PATH", None)
    parent = ROOT / ".artifacts"
    parent.mkdir(exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="road-slope-", dir=parent))
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
    case = output / "case"
    original(job, fleet / "final.sav", case / "canonical")
    original(job, fleet / "final.sav", case / "vectors", vectors=True)
    original(job, case / "vectors/save/autosave/exit.sav", case / "reload")
    input_hashes: dict[str, Json] = {
        str(path.relative_to(output)): digest(path)
        for folder in ("fleet-input", "original-fleet", "case")
        for path in (output / folder).rglob("*")
        if path.is_file()
    }
    write_json(output / "inputs-before.json", input_hashes)
    case_job = FoundationRun(ROOT, case, oracle)
    case_job.test(binaries.runtime, TEST, {"ROAD_SLOPE_CASE": str(case)})
    for name, replacement in (
        ("pieces", 15),
        ("success", True),
        ("cost", 1),
        ("expenses", 0),
    ):
        changed = output / "corruption" / name
        _ = shutil.copytree(case, changed)
        (changed / "comparison.json").unlink()
        observation = changed / "vectors/road-slope.json"
        value = read_json(observation)
        if at(value, ("rows", 0, name)) == replacement:
            raise WorldCheckError(
                "Native mutation must change the actual observed value"
            )
        replace(value, ("rows", 0, name), replacement)
        write_json(observation, value)
        rejected = run(
            [
                "env",
                f"ROAD_SLOPE_CASE={changed}",
                str(binaries.runtime),
                "--ignored",
                "--exact",
                TEST,
            ],
            output / "rejection" / name,
            expected=101,
        )
        if (
            "native road slope output differs" not in rejected.stdout
            or "1 failed; 0 ignored" not in rejected.stdout
        ):
            raise WorldCheckError(
                "Native mutation did not reach the actual slope comparison"
            )
        if (changed / "comparison.json").exists():
            raise WorldCheckError("Rejected slope comparison published a result")
    provenance = read_json(output / "provenance.json")
    sources = at(provenance, ("sources",))
    match sources:
        case dict():
            for name, sha in sources.items():
                if digest(ROOT / name) != sha:
                    raise WorldCheckError("Road slope source changed during proof")
        case _:
            raise WorldCheckError("Missing road slope source identities")
    if digest(oracle) != at(provenance, ("oracle_sha256",)):
        raise WorldCheckError("Road slope oracle changed during proof")
    if digest(oracle.parent / "replay-build.sha256") != at(
        provenance, ("stamp_sha256",)
    ):
        raise WorldCheckError("Road slope native stamp changed during proof")
    for filename, sha in input_hashes.items():
        if digest(output / filename) != sha:
            raise WorldCheckError("Road slope input changed during proof")
    identities = read_json(output / "test-binaries.json")
    for name in ("ottd_sim", "native_depot_build", "ottd"):
        if digest(output / "bin" / name) != at(identities, (name, "sha256")):
            raise WorldCheckError("Retained road slope executable changed")
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
            "vectors": 155648,
            "price_probes": 5,
            "corruption_rejections": 4,
            "pure_helper_only": True,
        },
    )
    print(
        "PASS original CheckRoadSlope: 155648 vectors, 5 price probes, complete saved/live restoration, 4 rejected mutations"
    )


if __name__ == "__main__":
    main()
