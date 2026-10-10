from __future__ import annotations

import hashlib
import json
from pathlib import Path

from scripts.currency_ci_baseline import bind_baseline, canonical_transport
from scripts.currency_ci_compare import controls, lifecycle, project_rows, state
from scripts.currency_ci_roster import API, LOAD
from scripts.depot_build_archive import bounded_paths
from scripts.grf_control_evidence import sequence, text
from scripts.language_ci_bindings import normalized
from scripts.language_ci_compare import (
    compare,
    control_projection,
    mapping,
    number,
    strip_baseline,
)
from scripts.strings_ci_evidence import membership
from scripts.world_check_support import Json, WorldCheckError, at, read_json


def stable_context(native: Json) -> Json:
    # Original unix/macos main seeds this nonsaved RNG from time(nullptr).
    # Bind its unchanged per-process value, retaining all raw context evidence.
    result = dict(mapping(native))
    seeds: list[Json] = []

    def context(value: Json) -> Json:
        fields = dict(mapping(value))
        pair = sequence(at(fields, ("interactive_random",)))
        if len(pair) != 2 or any(number(v) > 0xFFFFFFFF for v in pair):
            raise WorldCheckError("Invalid startup interactive RNG state")
        seeds.append(list(pair))
        fields["interactive_random"] = "$UNCHANGED_PROCESS_STARTUP_RNG"
        return fields

    match at(native, ("mode",)):
        case "separate-process-original-currency-api":
            for name in ("before", "after"):
                result[name] = context(at(native, (name,)))
        case "actual-loader-currency-owners":
            result["loads"] = [
                {**mapping(row), "context": context(at(row, ("context",)))}
                for row in sequence(at(native, ("loads",)))
            ]
            reload = at(native, ("reload_context",))
            if reload is not None:
                result["reload_context"] = {
                    **mapping(reload),
                    **{
                        name: context(at(reload, (name,)))
                        for name in ("before", "prepared", "after")
                    },
                }
        case _:
            raise WorldCheckError("Unknown currency observer mode")
    if not seeds:
        raise WorldCheckError("Missing currency process context")
    for seed in seeds[1:]:
        compare(seed, seeds[0])
    return result


def fingerprint(value: Json, roots: tuple[Path, Path, Path]) -> str:
    def paths(value: Json) -> Json:
        match value:
            case dict() as fields:
                return {key: paths(child) for key, child in fields.items()}
            case list() as values:
                return [paths(child) for child in values]
            case str():
                return (
                    normalized(value, roots) if "/" in value or "\\" in value else value
                )
            case None | bool() | int() | float():
                return value

    encoded = json.dumps(
        paths(canonical_transport(value, roots[2])),
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode()
    return hashlib.sha256(encoded).hexdigest()


def case(root: Path, directory: Path, oracle: Path, layout: Json, *, api: bool) -> Json:
    _ = bounded_paths(directory, {text(v) for v in sequence(at(layout, ("paths",)))})
    native = read_json(directory / "native/currency.json")
    bind_baseline(directory, oracle, native)
    compare(
        fingerprint(stable_context(native), (root, directory, oracle.parent)),
        at(layout, ("native_observables_sha256",)),
    )
    compare(at(native, ("arms",)), 1)
    compare(read_json(directory / "status.json"), 0)
    if api:
        compare(at(native, ("mode",)), "separate-process-original-currency-api")
        compare(at(native, ("consumptions",)), 1)
        compare(at(native, ("explicit_fixture_reset",)), rust=True)
        compare(at(native, ("before",)), at(native, ("after",)))
        rust = read_json(directory / "rust.json")
        compare(at(native, ("results",)), rust)
        count = controls(rust, read_json(directory / "controls.json"))
        result: Json = {"operations": len(sequence(rust)), "controls": count}
    else:
        compare(at(native, ("mode",)), "actual-loader-currency-owners")
        loads = sequence(at(native, ("loads",)))
        compare(at(native, ("consumptions",)), len(loads))
        if len(loads) not in (1, 2):
            raise WorldCheckError("Currency load count outside admitted domain")
        result = {
            "loads": [
                load_case(directory, load, index) for index, load in enumerate(loads)
            ]
        }
        original = read_json(directory / "native/control.json")
        compare(
            control_projection(original),
            read_json(directory / "load-0/rust-control.json"),
        )
    compare(result, at(layout, ("coverage",)))
    return result


def load_case(directory: Path, native: Json, index: int) -> Json:
    output = directory / f"load-{index}"
    rust = read_json(output / "rust.json")
    compare(project_rows(native, 2), rust)
    count = controls(rust, read_json(output / "controls.json"))
    phases = read_json(output / "lifecycle/rust.json")
    compare(lifecycle(native), phases)
    phase_count = controls(phases, read_json(output / "lifecycle/controls.json"))
    compare(state(native), read_json(output / "rust-final.json"))
    files = sequence(at(native, ("files",)))
    prefix = [number(at(row, ("config_grfid",))) for row in files[:2]]
    if len(prefix) != 2 or len(set(prefix)) != 2:
        raise WorldCheckError("Missing baseline file identities")
    final = strip_baseline({"files": files}, prefix)
    compare(
        at(final, ("files",)), at(read_json(output / "rust-control.json"), ("files",))
    )
    return {
        "events": len(sequence(rust)),
        "controls": count,
        "lifecycle_controls": phase_count,
    }


def validate_currency(root: Path, output: Path, oracle: Path, layout: Json) -> None:
    if at(layout, ("verified_native_corpus",)) is not True:
        raise WorldCheckError("Currency layout awaits actual complete native corpus")
    actual: dict[str, Json] = {"guards": 13}
    for group, names in (("api", API), ("load", LOAD)):
        summary = read_json(output / group / "summary.json")
        membership(summary, at(layout, (group, "summary")))
        compare(
            [text(at(row, ("case",))) for row in sequence(summary)],
            [str(v) for v in names],
        )
        cases = mapping(at(layout, (group, "cases")))
        compare([str(v) for v in sorted(cases)], [str(v) for v in sorted(names)])
        compare(
            [
                str(v)
                for v in sorted(
                    p.name for p in (output / group).iterdir() if p.is_dir()
                )
            ],
            [str(v) for v in sorted(names)],
        )
        values = [
            case(root, output / group / name, oracle, item, api=group == "api")
            for name, item in cases.items()
        ]
        actual[group] = coverage(values, api=group == "api")
    compare(actual, at(layout, ("totals",)))


def coverage(values: list[Json], *, api: bool) -> Json:
    if api:
        return {
            "cases": len(values),
            "operations": sum(number(at(row, ("operations",))) for row in values),
            "controls": sum(number(at(row, ("controls",))) for row in values),
        }
    loads = [load for row in values for load in sequence(at(row, ("loads",)))]
    return {
        "cases": len(values),
        "loads": len(loads),
        **{
            key: sum(number(at(row, (key,))) for row in loads)
            for key in ("events", "controls", "lifecycle_controls")
        },
    }
