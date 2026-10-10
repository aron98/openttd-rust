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

from scripts.context_ci_support import retain_native
from scripts.depot_build_archive import bounded_paths, package_raw
from scripts.empty_road_controls import controls
from scripts.empty_road_evidence import membership, natural_visits
from scripts.empty_road_provenance import (
    admitted,
    build,
    capture,
    verify_binaries,
    verify_sources,
)
from scripts.empty_road_run import EmptyRun
from scripts.empty_road_summary import summarize
from scripts.gameplay_foundations import digest, require_test
from scripts.grf_control_evidence import sequence, text
from scripts.grf_control_run import ControlRun
from scripts.language_ci_compare import compare
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


def main() -> None:
    if sys.argv[1:] not in ([], ["--capture"]):
        raise WorldCheckError("Empty-road complete producer refuses subset")
    forbidden = [
        key
        for key in os.environ
        if (
            key.startswith(("OTTD_", "EMPTY_ROAD_", "CARGO_PROFILE_"))
            and key != "OTTD_EMPTY_ROAD_ORACLE"
        )
        or key
        in {
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
            "RUSTC",
            "RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER",
            "CARGO_BUILD_TARGET",
            "CARGO_BUILD_RUSTFLAGS",
            "MallocScribble",
            "MallocPreScribble",
            "MallocNanoZone",
        }
    ]
    if forbidden:
        raise WorldCheckError("Empty-road inherited overrides refused")
    capture_only = bool(sys.argv[1:])
    root = Path(__file__).resolve().parents[1]
    layout = admitted(root, capture_only=capture_only)
    parent = root / ".artifacts"
    parent.mkdir(exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="empty-road-", dir=parent))
    target = Path(tempfile.mkdtemp(prefix="empty-road-target-", dir=parent))
    print(f"Artifacts: {output}", flush=True)
    oracle = Path(
        os.environ.get(
            "OTTD_EMPTY_ROAD_ORACLE", str(root / ".reference/snapshot-build/openttd")
        )
    ).resolve(strict=True)
    native_sha = digest(oracle)
    job = ControlRun(root, output, oracle)
    hashes = capture(root, output)
    write_json(output / "layout.json", layout)
    _ = job.run(
        "freshness",
        [
            "cmake",
            f"-DORACLE={oracle}",
            "-P",
            str(root / "scripts/check-replay-build.cmake"),
        ],
    )
    _ = job.run("rust-version", ["rustc", "--version", "--verbose"])
    _ = job.run("python-version", ["python3", "--version"])
    binaries = build(job, target)
    fixtures = read_json(root / "scripts/empty-road-fixtures.json")
    selectors = [text(value) for value in sequence(at(fixtures, ("selectors",)))]
    if len(selectors) != 6:
        raise WorldCheckError("Empty-road exact six-test roster required")
    membership(selectors, selectors)
    for selector in selectors:
        result = job.run(
            "unit-" + selector,
            [
                text(at(binaries, ("runner", "retained"))),
                "--exact",
                selector,
                "--nocapture",
            ],
        )
        require_test(result.stdout, selector)
    run = EmptyRun(job, binaries, fixtures)
    run.execute()
    natural_visits(output / "native")
    write_json(output / "coverage.json", summarize(run))
    controls(run, hashes)
    verify_sources(root, hashes)
    verify_binaries(output, binaries)
    compare(digest(oracle), native_sha)
    retain_native(output, oracle)
    write_json(
        output / "native-identity.json",
        {
            "path": str(oracle),
            "before_sha256": native_sha,
            "after_sha256": digest(oracle),
        },
    )
    members = sorted(
        str(path.relative_to(output)) for path in output.rglob("*") if path.is_file()
    )
    artifact_paths: list[Json] = []
    artifact_paths.extend(sorted([*members, "candidate-layout.json"]))
    write_json(
        output / "candidate-layout.json",
        {
            "fresh_complete_run_verified": False,
            "sources": hashes,
            "artifact_paths": artifact_paths,
        },
    )
    if not capture_only:
        _ = bounded_paths(
            output, {text(value) for value in sequence(at(layout, ("artifact_paths",)))}
        )
    package_raw(output)
    status = (
        "CAPTURE complete; layout unadmitted"
        if capture_only
        else "PASS empty-road CI admission"
    )
    _ = (output / "summary.txt").write_text(status + "\n")
    print(status, flush=True)


if __name__ == "__main__":
    main()
