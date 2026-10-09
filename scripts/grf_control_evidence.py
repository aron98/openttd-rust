# SPDX-License-Identifier: GPL-2.0-only
from __future__ import annotations

from pathlib import Path

from scripts.gameplay_foundations import digest
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


def sequence(value: Json) -> list[Json]:
    match value:
        case list():
            return value
        case _:
            raise WorldCheckError("Expected loader evidence array")


def text(value: Json) -> str:
    match value:
        case str():
            return value
        case _:
            raise WorldCheckError("Expected loader evidence string")


def pointer(value: Json, path: str) -> Json:
    for component in path.removeprefix("/").split("/"):
        value = at(value, (int(component) if isinstance(value, list) else component,))
    return value


def controls(rust: Json, values: Json) -> int:
    entries = sequence(values)
    identities: set[str] = set()
    for entry in entries:
        if at(entry, ("comparator_rejected",)) is not True:
            raise WorldCheckError("Loader comparator control did not reject")
        match entry:
            case {"pointer": str() as path, "before": before, "after": after}:
                if pointer(rust, path) != before or before == after:
                    raise WorldCheckError("Loader comparator mutation is ineffective")
                identity = path
            case {
                "operation": "swap",
                "first": str() as first,
                "second": str() as second,
            }:
                if pointer(rust, first) == pointer(rust, second):
                    raise WorldCheckError("Loader order control is ineffective")
                identity = "swap"
            case _:
                raise WorldCheckError("Invalid loader comparator control")
        if identity in identities:
            raise WorldCheckError("Duplicate loader comparator control")
        identities.add(identity)
    return len(entries)


def validate_paths(root: Path, layout: Json) -> None:
    expected = {text(name) for name in sequence(at(layout, ("paths",)))}
    actual = {str(path.relative_to(root)) for path in root.rglob("*") if path.is_file()}
    if actual != expected:
        raise WorldCheckError(
            "Loader evidence path set differs from complete 247-case layout"
        )
    for name in expected:
        path = root / name
        if path.is_symlink() or not path.resolve().is_relative_to(root.resolve()):
            raise WorldCheckError(f"Escaped loader evidence: {name}")
        if not path.stat().st_size and not name.endswith(
            ("/stdout.log", "/stderr.log")
        ):
            raise WorldCheckError(f"Empty substantive loader evidence: {name}")


def validate_inputs(directory: Path) -> None:
    raw = read_json(directory / "native/control.json")
    if at(raw, ("arms",)) != 1 or at(raw, ("consumptions",)) != 1:
        raise WorldCheckError("Original loader was not consumed exactly once")
    if len(sequence(at(raw, ("baseline_sources",)))) != 2:
        raise WorldCheckError("Missing original baseline sources")
    for name in ("inputs.sha256", "baseline-inputs.sha256"):
        for line in (directory / "native" / name).read_text().splitlines():
            expected_hash, path = line.split("  ", 1)
            source = Path(path)
            if expected_hash == "MISSING":
                if source.exists():
                    raise WorldCheckError("Missing source unexpectedly exists")
            elif digest(source) != expected_hash:
                raise WorldCheckError("Loader input provenance changed")


def validate(root: Path, layout_path: Path) -> tuple[int, int, int, int]:
    layout = read_json(layout_path)
    validate_paths(root, layout)
    events = decisions = configs = rejected = 0
    cases = sequence(at(layout, ("cases",)))
    if len(cases) != 247 or len({text(case) for case in cases}) != 247:
        raise WorldCheckError("Incomplete loader identity set")
    for case in cases:
        directory = root / text(case)
        rust = read_json(directory / "rust.json")
        native = read_json(directory / "native-normalized.json")
        if rust != native:
            raise WorldCheckError(f"Loader projection mismatch: {case}")
        rows = sequence(at(rust, ("events",)))
        events += len(rows)
        decisions += sum(at(row, ("kind",)) == "record" for row in rows)
        configs += len(sequence(at(rust, ("files",))))
        rejected += controls(rust, read_json(directory / "negative-controls.json"))
        validate_inputs(directory)
    totals = events, decisions, configs, rejected
    if totals != (8673, 6697, 1293, 4746):
        raise WorldCheckError(f"Loader coverage totals changed: {totals}")
    write_json(
        root.parent / "coverage.json",
        {
            "cases": cases,
            "events": events,
            "record_decisions": decisions,
            "final_configs": configs,
            "comparator_rejections": rejected,
        },
    )
    return totals
