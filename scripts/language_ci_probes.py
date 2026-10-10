from __future__ import annotations

from collections.abc import Callable
from copy import deepcopy
from pathlib import Path

from scripts.context_ci_support import sources
from scripts.grf_control_evidence import sequence, text
from scripts.language_ci_bindings import bound_inputs
from scripts.language_ci_evidence import observations, require_cases
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    read_json,
    replace,
    write_json,
)


def rejected(name: str, action: Callable[[], None]) -> Json:
    try:
        action()
    except WorldCheckError as error:
        return {"name": name, "diagnostic": str(error)}
    raise WorldCheckError(f"Corruption was admitted: {name}")


def modified_file(path: Path, raw: bytes, action: Callable[[], None]) -> None:
    original = path.read_bytes()
    try:
        _ = path.write_bytes(raw)
        action()
    finally:
        _ = path.write_bytes(original)


def live_probes(root: Path, output: Path, oracle: Path, layout: Json) -> None:
    directory = output / "corruption"
    directory.mkdir()
    case = output / "results/german"
    mutations: list[tuple[str, str, tuple[str | int, ...], Json]] = [
        ("selected", "native/language.json", ("catalog", "selected"), False),
        ("catalog", "native/language.json", ("catalog", "catalog"), []),
        ("context", "native/language.json", ("after_prepare",), None),
        ("maps", "rust-language.json", ("events",), []),
        ("control", "rust-control.json", ("events",), []),
        ("text", "native/language.json", ("translated", 0, "bytes"), [255]),
        ("table", "native/language.json", ("translated", 0, "table_after"), None),
        ("query", "native/language.json", ("translated", 0, "query"), "absent"),
    ]
    rows: list[Json] = []
    for name, file, pointer, value in mutations:
        source = case / file
        altered = deepcopy(read_json(source))
        if at(altered, pointer) == value:
            raise WorldCheckError("Live mutation would be ineffective")
        replace(altered, pointer, value)
        target = directory / f"{name}.json"
        write_json(target, altered)
        rows.append(
            rejected(
                name,
                lambda source=source, target=target: modified_file(
                    source, target.read_bytes(), lambda: check_observations(case)
                ),
            )
        )
    cases = [text(v) for v in sequence(at(layout, ("cases",)))]
    write_json(directory / "missing-case.json", [str(v) for v in cases[:-1]])
    rows.append(rejected("missing-case", lambda: require_cases(cases, cases[:-1])))
    wrong = deepcopy(layout)
    replace(wrong, ("sources", "Cargo.toml"), "0" * 64)
    write_json(directory / "changed-source.json", wrong)
    rows.append(rejected("source", lambda: sources(root, wrong)))
    pack = case / "pack-0/input.lng"
    changed = pack.read_bytes() + b"changed"
    _ = (directory / "changed-input.lng").write_bytes(changed)
    rows.append(
        rejected(
            "input",
            lambda: modified_file(
                pack,
                changed,
                lambda: bound_inputs(
                    case, at(layout, ("case_bindings", "german")), (root, case, oracle)
                ),
            ),
        )
    )
    write_json(directory / "rejections.json", rows)


def check_observations(directory: Path) -> None:
    _ = observations(directory)
