"""Re-read the entire paired corpus; summaries alone never establish admission."""

from __future__ import annotations

from pathlib import Path

from scripts.script_vm_provenance import select_executable

from . import binding_controls, corruption, matrix_receipts, native_receipts
from .baseline import FileInput
from .compare import checkpoint_words, compare_pairs, compare_runs, validate_inputs
from .events import crossing
from .events import validate as validate_events
from .matrix import Surface, ViewScope, rust_views
from .preparation import verify
from .protocol import CASES, VehicleId, labels, qualification_horizon
from .qualify import Qualified, daily
from .refusals import MUTATIONS
from .value import DECODE, array, field, integer, require, same, string


def cases(output: Path) -> tuple[Qualified, ...]:
    result: list[Qualified] = []
    for case in CASES:
        stem = case.name.replace("-", "_")
        prepared = output / (stem + "_prepare")
        receipt = DECODE((prepared / "results.json").read_text())
        verify(receipt, case)
        subject = VehicleId(integer(field(receipt, "subject")))
        require("returned ID portfolio", condition=subject == case.reservations)
        witness = string(field(receipt, "witness_label"))
        require(
            "witness label",
            condition=witness in labels(integer(field(receipt, "calls")))[1:-1],
        )
        found = output / (stem + "_discover")
        discovery = DECODE((found / "results.json").read_text())
        h = integer(field(discovery, "calls"))
        _ = crossing(found, discovery)
        result.append(
            Qualified(
                case,
                prepared / (witness + ".sav"),
                subject,
                h,
                qualification_horizon(h),
                output / (stem + "_trace"),
                output / (stem + "_off"),
            )
        )
    return tuple(result)


def validate_case(output: Path, case: Qualified) -> None:
    stem = case.case.name.replace("-", "_")
    matrix_receipts.verify(output, case)
    for suffix, calls in (
        ("zero", 0),
        ("discover", case.crossing),
        ("trace", case.horizon),
        ("off", case.horizon),
        ("original_from_rust", case.crossing),
    ):
        directory = output / (stem + "_" + suffix)
        result = DECODE((directory / "results.json").read_text())
        require(
            "actual complete native horizon",
            condition=field(result, "calls") == calls
            and field(result, "observer_failure") == "none",
        )
        validate_events(directory, result)
        _ = validate_inputs(
            directory, tuple(labels(calls)), checkpoint_words(directory, "initial")
        )
    zero = output / (stem + "_zero")
    require(
        "zero full identity",
        condition=compare_pairs(zero, zero, (("initial", "final"),)).strict_full_equal,
    )
    _ = compare_runs(case.traced, case.trace_off, case.horizon, case.horizon)
    _ = daily(case.traced, case.subject)
    _ = compare_runs(
        case.traced,
        output / (stem + "_original_from_rust"),
        case.horizon,
        case.crossing,
        case.crossing,
    )
    for suffix, count, offset, surface in (
        ("cli_zero", 0, 0, Surface.CLI),
        ("runtime_zero", 0, 0, Surface.RUNTIME),
        ("cli_crossing", case.crossing, 0, Surface.CLI),
        ("runtime_crossing", case.crossing, 0, Surface.RUNTIME),
        ("cli", case.horizon, 0, Surface.CLI),
        ("runtime", case.horizon, 0, Surface.RUNTIME),
        ("cli_from_original", case.crossing, case.crossing, Surface.CLI),
        ("runtime_from_original", case.crossing, case.crossing, Surface.RUNTIME),
        ("runtime_from_rust", case.crossing, case.crossing, Surface.RUNTIME),
        ("cli_from_rust", case.crossing, case.crossing, Surface.CLI),
    ):
        rust_views(
            case.traced,
            output / (stem + "_" + suffix),
            ViewScope(count, int(case.subject), surface, offset),
        )
    for mutation in MUTATIONS:
        directory = output / (stem + "_refuse_" + mutation)
        for suffix in ("world", "derived", "physical"):
            require(
                "whole horizon rollback changed",
                condition=same(
                    DECODE((directory / f"before.{suffix}.json").read_text()),
                    DECODE((directory / f"after.{suffix}.json").read_text()),
                ),
            )
        require(
            "CLI failure published", condition=not (directory / "cli-output").exists()
        )
        receipt = DECODE(
            (
                output
                / "logs"
                / (stem + "_refuse_" + mutation + "_cli")
                / "process.json"
            ).read_text()
        )
        require("CLI actual refusal exit", condition=field(receipt, "returncode") == 1)


def binaries(output: Path) -> None:
    rows = array(DECODE((output / "binaries.json").read_text()))
    require(
        "exact actual Cargo executable roster",
        condition=[field(row, "target") for row in rows]
        == ["ottd", "road_movement_ci", "road_movement"],
    )
    for row in rows:
        name, kind = string(field(row, "target")), string(field(row, "kind"))
        log = (
            output
            / "logs"
            / ("cargo-" + (name if name == "road_movement" else kind))
            / "stdout.log"
        )
        selected = select_executable(log.read_text(), name, kind, test=kind == "test")
        require(
            "Cargo reported original path",
            condition=str(selected) == field(row, "original"),
        )
        require(
            "Cargo stdout identity",
            condition=FileInput.capture(log).digest
            == field(row, "cargo_stdout_sha256"),
        )
        require(
            "retained selected executable",
            condition=FileInput.capture(output / "bin" / name).digest
            == field(row, "sha256"),
        )


def validate(output: Path) -> None:
    native_receipts.verify(output)
    binaries(output)
    binding_controls.verify(output)
    for case in cases(output):
        validate_case(output, case)
        corruption.verify(output, case)
