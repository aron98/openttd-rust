from __future__ import annotations

from pathlib import Path

from scripts.depot_build_archive import bounded_paths
from scripts.grf_control_evidence import sequence, text
from scripts.language_ci_compare import compare, control_projection, mapping, number
from scripts.strings_ci_compare import controls, identities
from scripts.world_check_support import Json, WorldCheckError, at, read_json


def membership(actual: Json, expected: Json) -> None:
    names = [text(at(row, ("case",))) for row in sequence(actual)]
    required = [text(at(row, ("case",))) for row in sequence(expected)]
    if not required or len(set(required)) != len(required) or names != required:
        raise WorldCheckError("Missing, extra, reordered or duplicate string cases")
    compare(actual, expected)


def case(directory: Path, layout: Json, *, api: bool) -> tuple[int, int]:
    _ = bounded_paths(
        directory, {text(path) for path in sequence(at(layout, ("paths",)))}
    )
    native = read_json(directory / "native/strings.json")
    rust = read_json(directory / ("rust.json" if api else "rust-strings.json"))
    for key in ("arms", "consumptions"):
        compare(at(native, (key,)), 1)
    if api:
        compare(at(native, ("mode",)), "separate-process-original-string-api")
        compare(at(native, ("context_before",)), at(native, ("context_after",)))
        compare(at(native, ("results",)), rust)
    else:
        compare(at(native, ("mode",)), "actual-stage-major-custom-strings")
        compare(at(native, ("events",)), rust)
        final = read_json(directory / "rust-final.json")
        compare(at(native, ("final_table",)), at(final, ("table",)))
        compare(at(native, ("translation_errors",)), at(final, ("translation_errors",)))
        compare(
            control_projection(read_json(directory / "native/control.json")),
            read_json(directory / "rust-control.json"),
        )
    mutations = read_json(directory / "controls.json")
    compare(identities(mutations), at(layout, ("control_identities_sha256",)))
    count = controls(rust, mutations)
    compare(count, at(layout, ("controls",)))
    compare(read_json(directory / "status.json"), 0)
    return len(sequence(rust)), count


def validate_strings(output: Path, layout: Json) -> None:
    totals: list[Json] = []
    for group, api, required in [("api", True, 10), ("load", False, 90)]:
        directory = output / group
        expected = sequence(at(layout, (group, "summary")))
        if len(expected) != required:
            raise WorldCheckError("Wrong fixed string corpus size")
        membership(read_json(directory / "summary.json"), expected)
        entries = mapping(at(layout, (group, "cases")))
        compare(
            [str(v) for v in sorted(entries)],
            [str(v) for v in sorted(text(at(row, ("case",))) for row in expected)],
        )
        roots = sorted(path.name for path in directory.iterdir() if path.is_dir())
        compare([str(v) for v in roots], [str(v) for v in sorted(entries)])
        events = rejected = 0
        for name, item in entries.items():
            rows, mutations = case(directory / name, item, api=api)
            events += rows
            rejected += mutations
        compare(events, number(at(layout, (group, "events"))))
        compare(rejected, number(at(layout, (group, "controls"))))
        totals.append(
            {"group": group, "cases": required, "events": events, "controls": rejected}
        )
    compare(totals, at(layout, ("totals",)))
