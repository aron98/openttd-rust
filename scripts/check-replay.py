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

from scripts.replay_controls import evidence_controls, output_controls
from scripts.replay_lifecycle import original_continuation, split_replay
from scripts.replay_matrix import ReplayMatrix
from scripts.world_check_support import ROOT, WorldCheckError, write_json

CASES = {
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
    selected = arguments.case or list(CASES)
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
    if not arguments.case:
        split_replay(matrix)
        original_continuation(matrix)
        output_controls(matrix)
        evidence_controls(matrix)
    write_json(
        artifacts / "summary.json",
        {
            "passed": True,
            "cases": selected,
            "full_matrix": not bool(arguments.case),
            "controls_and_resume": not bool(arguments.case),
            "native_binary": str(matrix.oracle),
            "rust_binary": str(matrix.cli),
        },
    )
    print(f"PASS replay matrix: {artifacts}")


if __name__ == "__main__":
    main()
