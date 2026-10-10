# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Fresh tree/terrain CI; capture records a candidate, never CI PASS."""

from __future__ import annotations

import argparse
import os
import sys
import tempfile
import tomllib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scripts.context_ci_support import retain_native, sources
from scripts.depot_build_archive import package_raw
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import sequence, text
from scripts.grf_control_run import ControlRun
from scripts.language_ci_compare import compare, mapping
from scripts.tree_ci_bindings import capture, source_names, verify
from scripts.tree_ci_controls import prepare as prepare_controls
from scripts.tree_ci_controls import validate as validate_controls
from scripts.tree_ci_evidence import (
    membership,
    native_case,
    normal_rust,
    observer_and_shore_pairs,
    reload_stability,
    split_resume,
)
from scripts.tree_ci_prepare import Preparation
from scripts.tree_ci_probes import source_probe, subset_probe
from scripts.tree_ci_run import TreeRun
from scripts.tree_ci_tests import execute, validate
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


def arguments() -> bool:
    parser = argparse.ArgumentParser(description=__doc__)
    _ = parser.add_argument("--capture", action="store_true")
    args = vars(parser.parse_args())
    capture_only = args.get("capture")
    if not isinstance(capture_only, bool):
        raise WorldCheckError("Invalid capture option")
    allowed = {"OTTD_TREE_ORACLE"}
    forbidden = [
        key
        for key in os.environ
        if (key.startswith(("TREE_", "OTTD_")) and key not in allowed)
        or key
        in {
            "RUSTFLAGS",
            "RUSTC",
            "RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER",
            "CARGO_ENCODED_RUSTFLAGS",
        }
    ]
    if forbidden:
        raise WorldCheckError(
            "Complete tree CI refuses selector/replay/observer/compiler overrides: "
            + ",".join(sorted(forbidden))
        )
    return capture_only


def inputs(root: Path, *, capture_only: bool) -> tuple[Json, Json]:
    layout = read_json(root / "scripts/tree-terrain/layout.json")
    if not capture_only:
        if at(layout, ("fresh_complete_run_verified",)) is not True:
            raise WorldCheckError(
                "Tree CI awaits an actual complete fresh run and reviewed layout"
            )
        compare(
            [*sorted(mapping(at(layout, ("source_hashes",))))], [*source_names(root)]
        )
        sources(root, {"sources": at(layout, ("source_hashes",))})
    recipes = read_json(root / "scripts/tree-terrain/recipes.json")
    upstream = tomllib.loads((root / "upstream.toml").read_text())
    compare(upstream.get("commit"), at(recipes, ("upstream",)))
    compare(upstream.get("savegame_version"), 362)
    for name, sha in mapping(at(recipes, ("tracked_inputs",))).items():
        compare(digest(root / name), sha)
    return layout, recipes


def main() -> None:
    capture_only = arguments()
    root = Path(__file__).resolve().parents[1]
    layout, recipes = inputs(root, capture_only=capture_only)
    parent = root / ".artifacts"
    parent.mkdir(exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="tree-terrain-", dir=parent))
    target = Path(tempfile.mkdtemp(prefix="tree-terrain-target-", dir=parent))
    print(f"Artifacts: {output}", flush=True)
    oracle = Path(
        os.environ.get(
            "OTTD_TREE_ORACLE", str(root / ".reference/snapshot-build/openttd")
        )
    ).resolve(strict=True)
    run = TreeRun(ControlRun(root, output, oracle), target)
    _ = run.job.run(
        "native-freshness-before",
        [
            "cmake",
            f"-DORACLE={oracle}",
            "-P",
            str(root / "scripts/check-replay-build.cmake"),
        ],
    )
    provenance = capture(root, output, oracle)
    write_json(output / "layout.json", layout)
    lib, cli = run.build()
    preparation = Preparation(run, recipes, lib, cli)
    entries = sequence(at(recipes, ("native_runs",)))
    for entry in entries:
        _ = preparation.native(text(at(entry, ("id",))))
    membership(
        sorted(preparation.completed), sorted(text(at(row, ("id",))) for row in entries)
    )
    native_bindings = [native_case(root, output, oracle, entry) for entry in entries]
    native: list[Json] = [
        {key: value for key, value in mapping(row).items() if key != "hashes"}
        for row in native_bindings
    ]
    write_json(output / "native-bindings.json", native_bindings)
    domains = sequence(at(recipes, ("domains",)))
    (output / "corpus/draft").mkdir()
    write_json(
        output / "corpus/draft/native-cases.json",
        {
            "schema_version": 1,
            "cases": [
                {
                    **mapping(row),
                    "directory": str(output / "corpus/runs" / text(at(row, ("id",)))),
                }
                for row in domains
            ],
        },
    )
    prepare_controls(output)
    selectors = read_json(root / "scripts/tree-terrain/selectors.json")
    execute(run, lib, selectors)
    rust = validate(run, lib, selectors)
    normal = normal_rust(output, domains)
    validate_controls(output)
    split_resume(output)
    observer_and_shore_pairs(output)
    reload_stability(output)
    run.zero(lib)
    source_probe(run, at(provenance, ("source_hashes",)))
    subset_probe(run, lib, text(at(selectors, ("normal_corpus", 0))))
    compare(
        [native_case(root, output, oracle, entry) for entry in entries], native_bindings
    )
    verify(root, output, oracle)
    _ = run.job.run(
        "native-freshness-after",
        [
            "cmake",
            f"-DORACLE={oracle}",
            "-P",
            str(root / "scripts/check-replay-build.cmake"),
        ],
    )
    retain_native(output, oracle)
    coverage = {
        "native_runs": len(native),
        "ordinary": normal,
        "rust_groups": rust,
        "trace_controls": 9,
        "coupled_raw_vectors": 8,
        "company15_trace_parity": False,
        "all_stage_gates": "open",
    }
    write_json(output / "coverage.json", coverage)
    observed: Json = {
        "fresh_complete_run_verified": False,
        "source_hashes": at(provenance, ("source_hashes",)),
        "native_membership": native,
        "rust_membership": rust,
        "artifact_paths": list[Json](
            sorted(str(p.relative_to(output)) for p in output.rglob("*") if p.is_file())
        ),
    }
    write_json(output / "observed-layout.json", observed)
    if not capture_only:
        for key in ("native_membership", "rust_membership", "artifact_paths"):
            compare(at(observed, (key,)), at(layout, (key,)))
    package_raw(output)
    summary = (
        "CAPTURE tree/terrain complete; pending review, not CI PASS"
        if capture_only
        else "PASS tree/terrain complete fresh CI admission"
    )
    _ = (output / "summary.txt").write_text(summary + "\n")
    print(summary, flush=True)


if __name__ == "__main__":
    main()
