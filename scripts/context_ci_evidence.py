# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through scripts/check-grf-context-ci.py.
from __future__ import annotations

from pathlib import Path
from typing import Final

from scripts.context_ci_support import exact
from scripts.depot_build_archive import bounded_paths
from scripts.grf_control_evidence import controls, sequence, text, validate_inputs
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json

CLOCK: Final = (
    "calendar_date",
    "calendar_fraction",
    "economy_date",
    "economy_fraction",
    "tick",
    "display",
)


def compare_context(native: Json, rust: Json) -> None:
    if not exact(at(native, ("before",)), at(native, ("restored",))):
        raise WorldCheckError("Context fixture restoration differs")
    if not exact(
        sequence(at(native, ("files",)))[2:], at(rust, ("files",))
    ) or not exact(at(native, ("globals",)), at(rust, ("globals",))):
        raise WorldCheckError("Final context globals/files differ")
    for field in (
        *CLOCK,
        "calendar_year",
        "economy_year",
        "random",
        "interactive_random",
    ):
        if not exact(
            at(native, ("prepared", field)), at(native, ("after_native", field))
        ):
            raise WorldCheckError("Original loader failed clock/RNG restoration")
    for field in CLOCK:
        if not exact(at(native, ("prepared", field)), at(rust, ("clock", field))):
            raise WorldCheckError("Rust final context clock differs")


def validate_context(output: Path, layout: Json) -> None:
    cases = [text(value) for value in sequence(at(layout, ("cases",)))]
    if len(cases) != 66 or len(set(cases)) != 66:
        raise WorldCheckError("Context case set must contain all 66 identities")
    guards = [text(value) for value in sequence(at(layout, ("guards",)))]
    if len(guards) != 18 or len(set(guards)) != 18:
        raise WorldCheckError("Context guard set must contain all 18 identities")
    _ = bounded_paths(
        output / "results", {text(value) for value in sequence(at(layout, ("paths",)))}
    )
    _ = bounded_paths(
        output / "guards",
        {text(value) for value in sequence(at(layout, ("guard_paths",)))},
    )
    expected_controls = {text(value) for value in sequence(at(layout, ("controls",)))}
    events = decisions = configs = rejected = 0
    for name in cases:
        directory = output / "results" / name
        rust = read_json(directory / "rust.json")
        if not exact(rust, read_json(directory / "native-normalized.json")):
            raise WorldCheckError("Original context trace differs")
        context = read_json(directory / "context.json")
        compare_context(context, read_json(directory / "rust-context.json"))
        records = sequence(at(rust, ("events",)))
        events += len(records)
        decisions += sum(at(row, ("kind",)) == "record" for row in records)
        configs += len(sequence(at(rust, ("files",))))
        mutations = read_json(directory / "context-controls.json")
        observed = {text(at(row, ("pointer",))) for row in sequence(mutations)}
        if observed != expected_controls or len(observed) != 30:
            raise WorldCheckError("Context corruption membership changed")
        rejected += controls(context, mutations)
        if exact(read_json(directory / "altered-trace.json"), rust):
            raise WorldCheckError("Context trace mutation is ineffective")
        validate_inputs(directory)
    if (events, decisions, configs, rejected) != (67828, 67300, 121, 1980):
        raise WorldCheckError("Context coverage totals changed")
    for guard in guards:
        receipt = output / "guards" / guard / "comparison.txt"
        if not receipt.read_text().startswith("PASS rejected via actual surface:"):
            raise WorldCheckError("Native context guard did not reject")
    names: list[Json] = list(cases)
    write_json(
        output / "coverage.json",
        {
            "cases": names,
            "events": events,
            "record_decisions": decisions,
            "final_configs": configs,
            "context_rejections": rejected,
            "trace_rejections": len(cases),
            "host_guards": 18,
        },
    )
