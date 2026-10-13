"""Bind every matrix invocation to its actual input origin and selected executable."""

from __future__ import annotations

from pathlib import Path

from .physical import compare
from .protocol import cli_plan, labels
from .qualify import Qualified
from .value import DECODE, field, require, same


def verify(output: Path, case: Qualified) -> None:
    stem = case.case.name.replace("-", "_")
    h, q = case.crossing, case.horizon
    native_h = case.traced / f"tick_{h}.sav"
    rust_h = output / (stem + "_runtime") / f"tick_{h}.sav"
    for suffix, source, calls in (
        ("cli_zero", case.save, 0),
        ("runtime_zero", case.save, 0),
        ("cli_crossing", case.save, h),
        ("runtime_crossing", case.save, h),
        ("cli", case.save, q),
        ("runtime", case.save, q),
        ("cli_from_original", native_h, h),
        ("runtime_from_original", native_h, h),
        ("cli_from_rust", rust_h, h),
        ("runtime_from_rust", rust_h, h),
    ):
        name = stem + "_" + suffix
        directory = output / name
        logs = output / "logs" / name
        process = DECODE((logs / "process.json").read_text())
        require(
            "actual Rust success receipt",
            condition=field(process, "returncode") == 0
            and field(process, "expected") == 0,
        )
        argv = DECODE((logs / "argv.json").read_text())
        if suffix.startswith("cli"):
            plan = output / (name + "-plan.json")
            require(
                "public CLI actual origin and executable",
                condition=same(
                    argv,
                    [
                        str(output / "bin/ottd"),
                        "replay-world",
                        str(source),
                        str(plan),
                        str(directory),
                    ],
                )
                and same(DECODE(plan.read_text()), cli_plan(calls)),
            )
        else:
            request = output / (name + "-request.json")
            require(
                "runtime actual origin and executable",
                condition=same(
                    argv,
                    [
                        str(output / "bin/road_movement_ci"),
                        "--ignored",
                        "--exact",
                        "capture_public_runtime",
                        "--nocapture",
                    ],
                )
                and same(
                    DECODE(request.read_text()),
                    {"input": str(source), "output": str(directory), "calls": calls},
                ),
            )
            environment = DECODE((logs / "environment.json").read_text())
            require(
                "runtime request environment",
                condition=field(environment, "OTTD_MOVEMENT_RUST_REQUEST")
                == str(request),
            )
    original = output / (stem + "_original_from_rust")
    invocation = DECODE((original / "invocation.json").read_text())
    require(
        "original continuation must consume public CLI half save",
        condition=field(invocation, "input")
        == str(output / (stem + "_cli") / f"tick_{h}.sav"),
    )
    for label in labels(h):
        position = h
        if label == "final":
            position += h
        elif label != "initial":
            position += int(label[5:])
        compare(
            DECODE((original / f"{label}.movement.json").read_text()),
            DECODE(
                (
                    output / (stem + "_runtime") / f"tick_{position}.physical.json"
                ).read_text()
            ),
            int(case.subject),
        )
