from __future__ import annotations

from collections.abc import Callable
from copy import deepcopy

from scripts.engine_specs_ci_evidence import validate
from scripts.engine_specs_ci_roster import PROBES, SELECTOR
from scripts.gameplay_foundations import log_name
from scripts.grf_control_evidence import sequence, text
from scripts.grf_control_run import ControlRun
from scripts.language_ci_compare import compare
from scripts.language_ci_probes import modified_file, rejected
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    read_json,
    replace,
    write_json,
)


def live_probes(job: ControlRun, layout: Json) -> None:
    directory = job.output / "corruption"
    directory.mkdir()
    target = job.output / "cases/api-dynamic-on"
    native = read_json(target / "native/specs.json")
    indices = [
        index
        for index, row in enumerate(sequence(at(native, ("events",))))
        if at(row, ("phase",)) == "api-command"
    ]
    if len(indices) != 12:
        raise WorldCheckError("Engine corruption baseline lacks twelve API commands")
    first, property_index = indices[0], indices[4]
    rows: list[Json] = []

    def check() -> None:
        validate(job.root, job.output, job.oracle, layout)

    def execute(name: str, action: Callable[[], None]) -> None:
        check()
        rows.append(rejected(name, action))
        check()

    mutations: list[tuple[str, str, tuple[str | int, ...], Json]] = [
        (
            "rust-owner",
            "rust.json",
            ("results", 0, "state", "owners", 116, "vehicle", "Road", "max_speed"),
            65535,
        ),
        (
            "native-reader",
            "native/specs.json",
            ("events", property_index, "detail", "remaining"),
            99,
        ),
        ("missing-command", "rust.json", ("results",), []),
        (
            "native-rng",
            "native/specs.json",
            ("events", first, "state", "context", "random"),
            [0, 0],
        ),
        (
            "baseline-path",
            "native/specs.json",
            ("baseline_sources", 0),
            "/unbound/openttd.grf",
        ),
    ]
    for name, filename, pointer, value in mutations:
        altered = deepcopy(read_json(target / filename))
        if at(altered, pointer) == value:
            raise WorldCheckError("Engine mutation is ineffective")
        replace(altered, pointer, value)
        artifact = directory / (name + ".json")
        write_json(artifact, altered)
        execute(
            name,
            lambda filename=filename, artifact=artifact: modified_file(
                target / filename, artifact.read_bytes(), check
            ),
        )
    paired_native, paired_rust = (
        deepcopy(native),
        deepcopy(read_json(target / "rust.json")),
    )
    replace(
        paired_native,
        ("events", first, "state", "owners", 116, "vehicle", "Road", "max_speed"),
        65535,
    )
    replace(
        paired_rust,
        ("results", 0, "state", "owners", 116, "vehicle", "Road", "max_speed"),
        65535,
    )
    write_json(directory / "paired-native.json", paired_native)
    write_json(directory / "paired-rust.json", paired_rust)
    execute(
        "paired-owner",
        lambda: modified_file(
            target / "native/specs.json",
            (directory / "paired-native.json").read_bytes(),
            lambda: modified_file(
                target / "rust.json",
                (directory / "paired-rust.json").read_bytes(),
                check,
            ),
        ),
    )
    invocation = target / "native/invocation.txt"
    raw = invocation.read_text()
    corrupted = (
        "\n".join(
            "ORACLE_SHA256=" + "0" * 64 if line.startswith("ORACLE_SHA256=") else line
            for line in raw.splitlines()
        )
        + "\n"
    )
    _ = (directory / "native-invocation.txt").write_text(corrupted)
    execute(
        "native-invocation",
        lambda: modified_file(invocation, corrupted.encode(), check),
    )
    argv = job.output / "logs/build/argv.json"
    unlocked = [value for value in sequence(read_json(argv)) if value != "--locked"]
    write_json(directory / "cargo-unlocked.json", unlocked)
    execute(
        "cargo-unlocked",
        lambda: modified_file(
            argv, (directory / "cargo-unlocked.json").read_bytes(), check
        ),
    )
    source = job.root / "reference/engine_specs.hpp"
    execute("source", lambda: modified_file(source, source.read_bytes() + b"\n", check))
    selector_argv = job.output / "logs" / log_name(SELECTOR) / "argv.json"
    changed = [
        "nonexistent_engine_test" if value == SELECTOR else value
        for value in sequence(read_json(selector_argv))
    ]
    write_json(directory / "selector.json", changed)
    execute(
        "selector",
        lambda: modified_file(
            selector_argv, (directory / "selector.json").read_bytes(), check
        ),
    )
    summary = job.output / "cases/summary.json"
    write_json(directory / "missing-case.json", sequence(read_json(summary))[:-1])
    execute(
        "missing-case",
        lambda: modified_file(
            summary, (directory / "missing-case.json").read_bytes(), check
        ),
    )
    compare(
        sorted(text(at(row, ("name",))) for row in rows) == sorted(PROBES), rust=True
    )
    write_json(directory / "summary.json", rows)
