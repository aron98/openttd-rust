# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 scripts/check-house-runtime.py --native ... --lifecycle ...
#      --controls ... --ready ... --output ... [--corpus existing-original-captures]
"""Private vanilla house proof through actual native and Rust complete state loops."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scripts.gameplay_foundations import digest
from scripts.house_native import HouseNative
from scripts.house_runtime_controls import corruptions, production
from scripts.house_runtime_proof import HouseProof
from scripts.house_runtime_recipe import prepare
from scripts.tree_growth_run import compare
from scripts.world_check_support import Json, write_json


class Arguments(argparse.Namespace):
    native: Path = Path()
    lifecycle: Path = Path()
    controls: Path = Path()
    ready: Path = Path()
    output: Path = Path()
    corpus: Path | None = None


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("native", "lifecycle", "controls", "ready", "output"):
        _ = parser.add_argument("--" + name, type=Path, required=True)
    _ = parser.add_argument("--corpus", type=Path)
    args = parser.parse_args(namespace=Arguments())
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    native = HouseNative(args.native.resolve())
    proof = HouseProof(native, args.lifecycle.resolve(), args.controls.resolve())
    ready = args.ready.resolve()
    corpus = args.corpus.resolve() if args.corpus is not None else output / "native"
    if args.corpus is None:
        prepare(native, ready, corpus)
    write_json(
        output / "inputs.json",
        {
            str(path): digest(path)
            for path in (native.binary, proof.lifecycle, proof.controls, ready)
        },
    )
    counts: dict[str, Json] = {}
    for case in (
        "original-6144",
        "multi-6144",
        "bitcount-8192",
        "scale15-8192",
        "original-year",
        "paused",
        "zero",
    ):
        proof.run(corpus / case, output / case)
        if case not in {"paused", "zero"}:
            counts[case] = production(corpus / case)
    proof.continuation(corpus, output)
    proof.controls_run(ready, corpus, output / "controls")
    corruptions(proof, corpus / "original-6144", output / "corruptions")
    for view in ("world", "derived", "houses"):
        compare(
            corpus / "original-6144" / f"final.{view}.json",
            corpus / "observer-off-6144" / f"final.{view}.json",
            output / f"observer-off-{view}.json",
        )
    write_json(output / "PASS.json", {"production": counts, "complete": True})


if __name__ == "__main__":
    main()
