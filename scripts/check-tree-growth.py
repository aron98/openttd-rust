# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 scripts/check-tree-growth.py
#      --native ... --cli ... --test ... --output ...
"""Private full-loop proof; native must be built with tree_growth.patch."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scripts.gameplay_foundations import digest
from scripts.tree_growth_proof import Binaries, check_case, lifecycle
from scripts.tree_growth_recipe import (
    corrupt_cursor,
    late_failure,
    pause_and_zero,
    prepare,
)
from scripts.tree_growth_run import command, native, plan
from scripts.world_check_support import write_json


class Arguments(argparse.Namespace):
    native: Path = Path()
    cli: Path = Path()
    test: Path = Path()
    output: Path = Path()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ["native", "cli", "test", "output"]:
        _ = parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args(namespace=Arguments())
    bins = Binaries(args.native.resolve(), args.cli.resolve(), args.test.resolve())
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    write_json(
        output / "binaries.json",
        {str(path): digest(path) for path in [bins.native, bins.cli, bins.test]},
    )
    prepared = prepare(bins, output)
    check_case(bins, prepared, output / "no-spread", 32768)
    lifecycle(output / "no-spread")
    corrupt_cursor(bins, output)
    frozen = native(
        bins.native,
        prepared,
        plan([command("tree_growth_setting", value=3)]),
        output / "prepare-no-growth",
        observe=False,
    )
    check_case(bins, frozen, output / "no-growth", 4096)
    pause_and_zero(bins, prepared, output)
    late_failure(bins, prepared, output)
    write_json(
        output / "complete.json",
        {
            "passed": True,
            "scope": (
                "temperate settings 0/3 grass/rough tree callbacks through WorldTick"
            ),
            "native_state_loop_calls": 36864,
        },
    )


if __name__ == "__main__":
    main()
