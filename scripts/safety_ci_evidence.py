# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through scripts/check-grf-safety-ci.py.
from __future__ import annotations

from copy import deepcopy
from pathlib import Path

from scripts.context_ci_support import exact
from scripts.depot_build_archive import bounded_paths
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import sequence, text
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    read_json,
    replace,
    write_json,
)


def compare_case(native: Json, rust: Json) -> None:
    if not exact(native, rust):
        raise WorldCheckError("Static safety native observables differ")


def compare_restoration(raw: Json) -> None:
    match raw:
        case {
            "before": before,
            "restored": restored,
            "files_before": files_before,
            "files_restored": files_restored,
        }:
            if not exact(before, restored) or not exact(files_before, files_restored):
                raise WorldCheckError("Safety observer changed original context/files")
        case _:
            raise WorldCheckError("Missing original safety restoration observations")


def require_cases(expected: list[str], observed: list[str]) -> None:
    if len(expected) != 893 or len(set(expected)) != 893 or expected != observed:
        raise WorldCheckError("Static safety requires complete ordered893 case set")


def input_hashes(output: Path, manifest: list[Json]) -> None:
    rows = (output / "results/native/inputs.sha256").read_text().splitlines()
    if len(rows) != len(manifest):
        raise WorldCheckError("Safety input hash membership differs")
    for row, case in zip(rows, manifest, strict=True):
        sha, name = row.split("  ", 1)
        path = Path(name)
        expected = output / "results" / f"{text(at(case, ('id',)))}.grf"
        if path != expected or text(at(case, ("path",))) != name or digest(path) != sha:
            raise WorldCheckError("Safety input path/source changed")


def validate_safety(output: Path, layout: Json) -> None:
    cases = [text(value) for value in sequence(at(layout, ("cases",)))]
    results = output / "results"
    _ = bounded_paths(results, {text(v) for v in sequence(at(layout, ("paths",)))})
    _ = bounded_paths(
        output / "guards", {text(v) for v in sequence(at(layout, ("guard_paths",)))}
    )
    manifest = sequence(read_json(results / "manifest.json"))
    require_cases(cases, [text(at(row, ("id",))) for row in manifest])
    raw = read_json(results / "native/safety.json")
    compare_restoration(raw)
    native = sequence(at(raw, ("scans",)))
    require_cases(cases, [text(at(row, ("id",))) for row in native])
    coverage = read_json(results / "coverage.json")
    require_cases(cases, [text(v) for v in sequence(at(coverage, ("ids",)))])
    input_hashes(output, manifest)
    decisions = rejected = 0
    for name, observed in zip(cases, native, strict=True):
        directory = results / name
        rust = read_json(directory / "rust.json")
        compare_case(observed, read_json(directory / "native.json"))
        compare_case(observed, rust)
        decisions += len(sequence(at(rust, ("decisions",))))
        for path in sorted(directory.glob("negative-*.json")):
            altered = read_json(path)
            try:
                compare_case(observed, altered)
            except WorldCheckError:
                rejected += 1
            else:
                raise WorldCheckError("Ineffective safety comparator mutation")
    names: list[Json] = list(cases)
    if (decisions, rejected) != (1867, 13794) or not exact(
        coverage,
        {
            "ids": names,
            "cases": 893,
            "decisions": decisions,
            "comparator_rejections": rejected,
        },
    ):
        raise WorldCheckError("Static safety coverage totals changed")
    write_json(
        output / "coverage.json",
        {
            "cases": names,
            "record_decisions": decisions,
            "comparator_rejections": rejected,
            "host_guards": 10,
        },
    )


def live_probes(output: Path, layout: Json) -> None:
    directory = output / "corruption"
    directory.mkdir()
    native = read_json(output / "results/parameter-00-v1/native.json")
    rejected: list[Json] = []
    mutations: list[tuple[str, tuple[str | int, ...], Json]] = [
        ("status-type", ("status",), False),
        ("unsafe", ("unsafe",), True),
        ("admission", ("accepted",), False),
        ("identity", ("identity", "grfid"), 0),
        ("checksum", ("identity", "md5"), []),
        ("record", ("decisions", 1, "action"), 3),
        ("consumed", ("decisions", 1, "consumed"), 255),
        ("skip", ("decisions", 1, "skip"), -1),
    ]
    for name, path, value in mutations:
        wrong = deepcopy(native)
        replace(wrong, path, value)
        write_json(directory / f"{name}.json", wrong)
        try:
            compare_case(native, wrong)
        except WorldCheckError as error:
            rejected.append({"name": name, "diagnostic": str(error)})
        else:
            raise WorldCheckError("Live safety corruption was admitted")
    raw = read_json(output / "results/native/safety.json")
    for name, path in [("clock", ("restored", "tick")), ("files", ("files_restored",))]:
        wrong = deepcopy(raw)
        replace(wrong, path, "corrupt")
        write_json(directory / f"{name}.json", wrong)
        try:
            compare_restoration(wrong)
        except WorldCheckError as error:
            rejected.append({"name": name, "diagnostic": str(error)})
        else:
            raise WorldCheckError("Live native restoration corruption was admitted")
    cases = [text(value) for value in sequence(at(layout, ("cases",)))]
    write_json(directory / "missing-case.json", list(cases[:-1]))
    try:
        require_cases(cases, cases[:-1])
    except WorldCheckError as error:
        rejected.append({"name": "missing-case", "diagnostic": str(error)})
    else:
        raise WorldCheckError("Live incomplete safety matrix was admitted")
    write_json(directory / "rejections.json", rejected)
