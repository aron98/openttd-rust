# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Private temperate spreading proof through the complete original state loop."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scripts.gameplay_foundations import digest
from scripts.tree_growth_proof import Binaries
from scripts.tree_growth_recipe import pause_and_zero
from scripts.tree_growth_run import run
from scripts.tree_spread_assert import check_assertion
from scripts.tree_spread_counter import probe
from scripts.tree_spread_mixed import check_mixed
from scripts.tree_spread_proof import check_case, lifecycle
from scripts.tree_spread_recipe import large, moving, setting
from scripts.world_check_support import write_json


class Arguments(argparse.Namespace):
    native: Path = Path()
    assertion_native: Path = Path()
    native_source: Path = Path()
    cli: Path = Path()
    test: Path = Path()
    runtime: Path = Path()
    libtest: Path = Path()
    seed: Path = Path()
    output: Path = Path()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    names = [
        "native",
        "assertion-native",
        "native-source",
        "cli",
        "test",
        "runtime",
        "libtest",
        "seed",
        "output",
    ]
    for name in names:
        _ = parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args(namespace=Arguments())
    bins = Binaries(args.native.resolve(), args.cli.resolve(), args.test.resolve())
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    write_json(
        output / "bindings.json",
        {
            "files": {
                str(path.resolve()): digest(path)
                for path in [
                    args.native,
                    args.assertion_native,
                    args.cli,
                    args.test,
                    args.runtime,
                    args.libtest,
                    args.seed,
                ]
            },
            "scope": (
                "temperate settings 1/2 only; private driver; not normal CI admission"
            ),
        },
    )
    one = setting(bins.native, args.seed.resolve(), output / "setting-1", 1)
    two = setting(bins.native, args.seed.resolve(), output / "setting-2", 2)
    check_case(bins, one, output / "rainforest-only", 4096)
    check_case(bins, two, output / "all-spread", 32768)
    lifecycle(output / "all-spread")
    generated = large(bins.native, output / "large-generated")
    large_seed = setting(bins.native, generated, output / "large-setting", 2)
    check_case(bins, large_seed, output / "large", 128)
    pause_and_zero(bins, two, output / "controls")
    prepared = moving(bins.native, two, output / "mixed-prepare")
    check_mixed(bins.native, args.runtime.resolve(), prepared, output / "mixed")
    check_assertion(bins, args.assertion_native.resolve(), output / "boundary")
    rows = probe(args.native_source.resolve(), output / "counter-probe")
    run(
        [
            str(args.libtest.resolve()),
            "--ignored",
            "--exact",
            "world_ticks::trees::counter_tests::source_bound_native_counter_arithmetic",
            "--nocapture",
        ],
        output / "counter-rust",
        env={"OTTD_TREE_COUNTER_PROBE": str(rows)},
    )
    write_json(
        output / "PASS.json",
        {
            "passed": True,
            "whole_loop_temperate_settings": [1, 2],
            "full_world_map_sizes": [64, 512],
            "scale256_isolated_arithmetic_only": True,
            "larger_climates_water_and_command_parity_remain_unsupported": True,
        },
    )


if __name__ == "__main__":
    main()
