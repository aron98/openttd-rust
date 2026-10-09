# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 -m scripts.check-replay --artifacts FRESH_PATH
"""Execute supported replay parity through the public CLI and pinned native engine."""

from __future__ import annotations

import argparse
import hashlib
import sys
from pathlib import Path

if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.level_replay import verify_level
from scripts.replay_controls import evidence_controls, output_controls
from scripts.replay_lifecycle import original_continuation, split_replay
from scripts.replay_matrix import ReplayMatrix
from scripts.terraform_replay import verify_terraform
from scripts.world_check_support import ROOT, Json, WorldCheckError, write_json

CASES = {
    "level-basic": "populated",
    "level-diagonal": "populated",
    "level-diagonal-edges": "populated",
    "level-limits": "populated",
    "level-limit-one": "populated",
    "level-limit-zero": "populated",
    "level-cash": "populated",
    "level-partial-cash": "populated",
    "level-partial-diagonal": "populated",
    "level-exact-cash": "populated",
    "level-bounds": "populated",
    "level-freeform": "populated",
    "level-gates": "populated",
    "level-tunnel": "populated",
    "level-resume": "populated",
    "terraform-basic": "populated",
    "terraform-cascade": "populated",
    "terraform-masks": "populated",
    "terraform-limits": "populated",
    "terraform-bounds": "populated",
    "terraform-freeform": "populated",
    "terraform-cash": "populated",
    "terraform-exact-cash": "populated",
    "terraform-gates": "populated",
    "terraform-tunnel": "populated",
    "terraform-resume": "populated",
    "commands": "populated",
    "populated-road": "populated",
    "construction-continue": "populated",
    "ticks": "clear",
    "month": "month",
    "quarter": "quarter",
    "year": "year",
    "leap": "leap",
    "wrap": "wrap",
    "growth": "growth",
    "town-history": "town-history",
    "low-cash": "low-cash",
    "pause-gates": "pause-gates",
    "clear-limit": "clear-limit",
    "nested-name": "nested-name",
    "large-loan": "large-loan",
    "loan-command": "loan-command",
    "town-sum": "town-sum",
}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--oracle", type=Path, default=ROOT / ".reference/snapshot-build/openttd"
    )
    parser.add_argument(
        "--original", type=Path, default=ROOT / ".reference/build/openttd"
    )
    parser.add_argument("--ottd", type=Path, default=ROOT / "target/debug/ottd")
    parser.add_argument("--artifacts", type=Path, required=True)
    parser.add_argument("--case", action="append", choices=sorted(CASES))
    arguments = parser.parse_args()
    artifacts = arguments.artifacts.resolve()
    if artifacts.exists():
        raise WorldCheckError(f"Artifact directory already exists: {artifacts}")
    artifacts.mkdir(parents=True)
    matrix = ReplayMatrix(
        arguments.ottd.resolve(),
        arguments.oracle.resolve(),
        arguments.original.resolve(),
        artifacts,
    )
    selected: list[str] = arguments.case or list(CASES)
    write_json(
        artifacts / "provenance.json",
        {
            "upstream_commit": "14ec60f248547d4d062a1160f0fc26d742319888",
            "native_sha256": hashlib.sha256(matrix.oracle.read_bytes()).hexdigest(),
            "rust_sha256": hashlib.sha256(matrix.cli.read_bytes()).hexdigest(),
            "original_sha256": hashlib.sha256(matrix.original.read_bytes()).hexdigest(),
        },
    )
    for name in selected:
        _ = matrix.scenario(name, CASES[name])
        print(f"PASS {name}", flush=True)
    verify_terraform(matrix, selected)
    verify_level(matrix, selected)
    if not arguments.case:
        split_replay(matrix)
        original_continuation(matrix)
        output_controls(matrix)
        evidence_controls(matrix)
    case_values: list[Json] = list(selected)
    summary: dict[str, Json] = {
        "passed": True,
        "cases": case_values,
        "full_matrix": not bool(arguments.case),
        "controls_and_resume": not bool(arguments.case),
        "terraform_controls": "terraform-basic" in selected,
        "level_controls": "level-basic" in selected
        and "level-partial-cash" in selected,
        "level_resume_and_continuation": "level-resume" in selected,
        "terraform_resume_and_continuation": "terraform-resume" in selected,
        "native_binary": str(matrix.oracle),
        "rust_binary": str(matrix.cli),
    }
    write_json(artifacts / "summary.json", summary)
    print(f"PASS replay matrix: {artifacts}")


if __name__ == "__main__":
    main()
