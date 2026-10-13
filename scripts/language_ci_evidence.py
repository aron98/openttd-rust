from __future__ import annotations

from pathlib import Path
from typing import Final

from scripts.depot_build_archive import bounded_paths
from scripts.grf_control_evidence import sequence, text, validate_inputs
from scripts.language_ci_compare import (
    compare,
    control_projection,
    mapping,
    mutations,
    number,
)
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json

EMPTY_INPUTS: Final = frozenset(
    {
        (
            "catalog-empty/pack-0/input.lng",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
    }
)


def require_cases(expected: list[str], actual: list[str]) -> None:
    if len(expected) != 110 or len(set(expected)) != 110 or expected != actual:
        raise WorldCheckError("Language requires complete ordered110 case identities")


def observations(directory: Path) -> dict[str, Json]:
    raw = read_json(directory / "native/language.json")
    rust = read_json(directory / "rust-language.json")
    compare(at(raw, ("before",)), at(raw, ("after_prepare",)))
    compare(at(raw, ("phase",)), "actual-stage-major-language")
    selected = at(rust, ("selected",))
    packs = [
        row
        for row in sequence(at(rust, ("catalog",)))
        if at(row, ("language",)) == selected
    ]
    if len(packs) != 1:
        raise WorldCheckError("Missing unique selected language pack")
    expected: dict[str, Json] = {
        "control": control_projection(read_json(directory / "native/control.json")),
        "selected": at(raw, ("catalog", "selected")),
        "selected-header": at(raw, ("catalog", "selected_header")),
        "selected-strings": at(raw, ("catalog", "selected_strings")),
        "catalog": [
            at(row, ("header",)) for row in sequence(at(raw, ("catalog", "catalog")))
        ],
        "admissions": at(raw, ("catalog", "admissions")),
    }
    compare(expected["control"], read_json(directory / "rust-control.json"))
    compare(expected["selected"], selected)
    compare(expected["selected-header"], packs[0])
    compare(
        expected["selected-strings"],
        sum(number(v) for v in sequence(at(packs[0], ("tables",)))),
    )
    compare(expected["catalog"], at(rust, ("catalog",)))
    compare(expected["admissions"], at(rust, ("admissions",)))
    for name, value in [
        ("pack_version", 0x2AD109AB),
        ("plural_rules", 15),
        ("plural_forms", 5),
    ]:
        expected[name] = at(raw, ("catalog", name))
        compare(expected[name], value)
    events: list[Json] = []
    for row in sequence(at(raw, ("events",))):
        event = dict(mapping(row))
        files = sequence(at(event, ("files",)))
        if len(files) < 2:
            raise WorldCheckError("Missing baseline language maps")
        event["files"] = files[2:]
        events.append(event)
    compare(events, read_json(directory / "native-events.json"))
    compare(events, at(rust, ("events",)))
    expected["events"] = events
    text_observations(directory, raw, expected)
    return expected


def text_observations(directory: Path, raw: Json, expected: dict[str, Json]) -> None:
    queries = sequence(
        at(read_json(directory / "manifest.json"), ("language", "queries"))
    )
    observed = sequence(at(raw, ("translated",)))
    names = [text(at(query, ("id",))) for query in queries]
    if len(names) != len(set(names)) or len(queries) != len(observed):
        raise WorldCheckError("Language query identity membership differs")
    actual: list[str] = []
    for row in observed:
        name = text(at(row, ("query",)))
        actual.append(name)
        matches = [query for query in queries if at(query, ("id",)) == name]
        if len(matches) != 1:
            raise WorldCheckError("Unexpected language query")
        query = matches[0]
        for key in ("stage", "file", "line"):
            compare(at(row, (key,)), at(query, (key,)))
        compare(at(row, ("table_before",)), at(row, ("table_after",)))
        value = {
            key: at(row, (key,)) for key in ("query", "stage", "file", "line", "bytes")
        }
        compare(value, read_json(directory / f"text-{name}.json"))
        expected[f"text-{name}"] = value
    if len(set(actual)) != len(names) or set(actual) != set(names):
        raise WorldCheckError("Duplicate language text observation")


def controls_for_case(directory: Path, layout: Json) -> tuple[int, int, int]:
    expected = observations(directory)
    membership = mapping(at(layout, ("controls", directory.name)))
    if set(membership) != set(expected):
        raise WorldCheckError("Language control group membership differs")
    rejected = 0
    for name, value in expected.items():
        rows = read_json(directory / f"{name}-controls.json")
        identities = [
            at(row, ("pointer",))
            if "pointer" in mapping(row)
            else at(row, ("operation",))
            for row in sequence(rows)
        ]
        compare(identities, membership[name])
        rejected += mutations(value, rows)
    raw = read_json(directory / "native/language.json")
    return (
        len(sequence(at(raw, ("events",)))),
        len(sequence(at(raw, ("translated",)))),
        rejected,
    )


def validate_language(output: Path, layout: Json) -> None:
    cases = [text(value) for value in sequence(at(layout, ("cases",)))]
    root = output / "results"
    _ = bounded_paths(
        root,
        {text(v) for v in sequence(at(layout, ("paths",)))},
        empty_inputs=EMPTY_INPUTS,
    )
    _ = bounded_paths(
        output / "guards", {text(v) for v in sequence(at(layout, ("guard_paths",)))}
    )
    require_cases(cases, [text(v) for v in sequence(read_json(root / "cases.json"))])
    events = queries = rejected = 0
    for name in cases:
        directory = root / name
        compare(read_json(directory / "status.json"), 0)
        validate_inputs(directory)
        event_count, query_count, mutation_count = controls_for_case(directory, layout)
        events += event_count
        queries += query_count
        rejected += mutation_count
    if (events, queries, rejected) != (1593, 2560, 73604):
        raise WorldCheckError("Language complete coverage totals differ")
    write_json(
        output / "coverage.json",
        {
            "cases": [str(name) for name in cases],
            "case_count": 110,
            "native_events": events,
            "text_queries": queries,
            "comparator_rejections": rejected,
            "guards": 13,
        },
    )
