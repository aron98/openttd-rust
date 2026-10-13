#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-only
"""Run a source-bound mixed scenario, separate from normal movement CI admission."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.tree_growth_mixed_proof import proof


class Arguments(argparse.Namespace):
    native: Path = Path()
    runtime: Path = Path()
    seed: Path = Path()
    output: Path = Path()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("native", "runtime", "seed", "output"):
        _ = parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args(namespace=Arguments())
    proof(
        args.native.resolve(),
        args.runtime.resolve(),
        args.seed.resolve(),
        args.output.resolve(),
    )


if __name__ == "__main__":
    main()
