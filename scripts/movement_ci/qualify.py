"""Original-only qualification graph with measured crossing and full selector period."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from .compare import (
    INTERACTIVE,
    checkpoint_words,
    compare_pairs,
    compare_runs,
    validate_inputs,
)
from .events import crossing
from .native import Native, write
from .preparation import verify
from .protocol import Case, VehicleId, labels, loaded, prepare, qualification_horizon
from .value import DECODE, Json, array, field, integer, require, string


@dataclass(frozen=True, slots=True)
class Qualified:
    case: Case
    save: Path
    subject: VehicleId
    crossing: int
    horizon: int
    traced: Path
    trace_off: Path


def daily(directory: Path, subject: VehicleId) -> Json:
    required = {
        "calendar_selected",
        "economy_selected",
        "road_calendar_day",
        "road_economy_day",
    }
    selected: list[Json] = []
    with (directory / "events.jsonl").open() as stream:
        for line in stream:
            event = DECODE(line)
            phase = string(field(event, "phase"))
            if (
                phase in required
                and field(event, "edge") == "enter"
                and field(event, "vehicle") == subject
            ):
                family = (
                    "calendar_fract"
                    if phase in {"calendar_selected", "road_calendar_day"}
                    else "economy_fract"
                )
                require(
                    "native ID-staggered selector fraction",
                    condition=integer(field(field(event, "live"), family))
                    == int(subject) % 74,
                )
                selected.append(
                    {
                        "phase": phase,
                        "call": field(event, "call"),
                        "subject": int(subject),
                        "fraction": field(field(event, "live"), family),
                    }
                )
    require(
        "daily selector branch witnesses missing",
        condition={string(field(row, "phase")) for row in selected} == required,
    )
    return selected


def qualify(native: Native, case: Case, seed: Path) -> Qualified:
    stem = case.name.replace("-", "_")
    prepared = native.run(
        stem + "_prepare", seed, prepare(case), "native_moving_witness"
    )
    result = DECODE((prepared / "results.json").read_text())
    verify(result, case)
    subject = VehicleId(integer(field(result, "subject")))
    require("original returned target differs", condition=subject == case.reservations)
    witness = string(field(result, "witness_label"))
    require(
        "unsafe witness label",
        condition=witness.startswith("tick_") and witness[5:].isdigit(),
    )
    save = prepared / (witness + ".sav")
    zero = native.run(
        stem + "_zero",
        save,
        loaded("replay", subject, 0, trace=True),
        "native_completed",
    )
    initial = checkpoint_words(zero, "initial")
    _ = validate_inputs(zero, tuple(labels(0)), initial)
    strict = compare_pairs(zero, zero, (("initial", "final"),))
    require("zero-call native state changed", condition=strict.strict_full_equal)
    discovered = native.run(
        stem + "_discover",
        save,
        loaded("discover", subject, 0, trace=True),
        "native_crossing_witness",
    )
    found = DECODE((discovered / "results.json").read_text())
    horizon = integer(field(found, "calls"))
    entries, rng_calls = crossing(discovered, found)
    qualification = qualification_horizon(horizon)
    traced = native.run(
        stem + "_trace",
        save,
        loaded("replay", subject, qualification, trace=True),
        "native_completed",
    )
    off = native.run(
        stem + "_off",
        save,
        loaded("replay", subject, qualification, trace=False),
        "native_completed",
    )
    compared = compare_runs(traced, off, qualification, qualification)
    checkpoints = array(
        field(DECODE((traced / "results.json").read_text()), "checkpoints")
    )
    require(
        "native checkpoint labels",
        condition=[field(item, "label") for item in checkpoints]
        == labels(qualification),
    )
    receipt: Json = {
        "case": case.name,
        "subject": int(subject),
        "crossing": horizon,
        "qualification": qualification,
        "crossing_entry_results": entries,
        "one_choice_rng_calls": rng_calls,
        "daily": daily(traced, subject),
        "comparison_policy": (
            "bounded-saved-gameplay-all-other-captured-runtime-"
            "independent-interactive-inputs-v2"
        ),
        "independent_paths": [list(path) for path in sorted(INTERACTIVE)],
        "compared_views": compared.pairs,
        "strict_full_runtime_equal": compared.strict_full_equal,
        "raw_independent_differences": [
            {"path": list(row.path), "left": row.left, "right": row.right}
            for row in compared.independent_input_differences
        ],
    }
    write(native.output / (stem + "_qualification.json"), receipt)
    return Qualified(case, save, subject, horizon, qualification, traced, off)
