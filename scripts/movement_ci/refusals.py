"""Actual public runtime rollback and CLI non-publication controls."""

from __future__ import annotations

from typing import Final

from scripts.gameplay_foundations import require_test

from .baseline import FileInput
from .native import write
from .qualify import Qualified
from .rust import Rust
from .value import DECODE, field, integer, require, same

MUTATIONS: Final = (
    "other_side",
    "direction",
    "stopped",
    "uncovered_engine",
    "turn",
    "offset_month",
    "daily_breakdown",
    "zero_speed",
    "active_order",
    "active_breakdown",
    "saved_path",
    "realistic_acceleration",
)
SELECTOR: Final = "controls::whole_horizon_refusal"


def run(rust: Rust, case: Qualified) -> None:
    stem = case.case.name.replace("-", "_")
    source = FileInput.capture(case.save)
    for mutation in MUTATIONS:
        rust.harness.verify()
        name = stem + "_refuse_" + mutation
        output = rust.job.output / name
        request = rust.job.output / (name + "-request.json")
        write(
            request,
            {
                "input": str(case.save),
                "output": str(output),
                "subject": int(case.subject),
                "mutation": mutation,
                "crossing": case.crossing,
            },
        )
        result = rust.job.run(
            name,
            [
                str(rust.harness.retained.path),
                "--ignored",
                "--exact",
                SELECTOR,
                "--nocapture",
            ],
            {"OTTD_MOVEMENT_RUST_CONTROL": str(request)},
        )
        require_test(result.stdout, SELECTOR)
        source.verify()
        rust.harness.verify()
        for suffix in ("world", "derived", "physical"):
            require(
                "rollback publication mismatch",
                condition=same(
                    DECODE((output / f"before.{suffix}.json").read_text()),
                    DECODE((output / f"after.{suffix}.json").read_text()),
                ),
            )
        refusal = DECODE((output / "refusal.json").read_text())
        horizon = integer(field(refusal, "horizon"))
        negative = FileInput.capture(output / "negative.sav")
        plan = output / "cli-plan.json"
        write(
            plan,
            {
                "schema_version": 1,
                "actions": [{"op": "tick", "ordinal": 0, "count": horizon}],
            },
        )
        unpublished = output / "cli-output"
        rust.cli.verify()
        _ = rust.job.run(
            name + "_cli",
            [
                str(rust.cli.retained.path),
                "replay-world",
                str(negative.path),
                str(plan),
                str(unpublished),
            ],
            expected=1,
        )
        require("failed CLI published output", condition=not unpublished.exists())
        negative.verify()
        rust.cli.verify()
        write(
            output / "cli-refusal.json",
            {
                "exit_code": 1,
                "output_absent": True,
                "input_sha256": negative.digest,
                "whole_horizon": horizon,
            },
        )
