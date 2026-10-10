from __future__ import annotations

from pathlib import Path

from scripts.cargo_identity_ci_guards import native_argv
from scripts.cargo_identity_ci_projection import indices, inherited_mask, projection
from scripts.cargo_identity_ci_roster import (
    CASES,
    COMPILER_INPUT,
    EVENT_COUNTS,
    FOCUSED,
    INCLUDED_COUNTS,
    SELECTOR,
)
from scripts.context_ci_support import process, sources, verify_identity
from scripts.currency_ci_baseline import bind_baseline
from scripts.currency_ci_bindings import cargo_binding, invocations
from scripts.engine_specs_ci_evidence import fingerprint
from scripts.gameplay_foundations import log_name
from scripts.grf_control_evidence import sequence, text
from scripts.grf_control_run import ControlRun
from scripts.language_ci_compare import compare, mapping
from scripts.world_check_support import Json, WorldCheckError, at, read_json


def case(
    job: ControlRun, definition: Json, expected: Json, total: int, included_count: int
) -> None:
    name = text(at(definition, ("name",)))
    directory = job.output / "cases" / name
    native = read_json(directory / "native/cargo.json")
    control = read_json(directory / "native/control.json")
    manifest = read_json(directory / "manifest.json")
    compare(manifest, at(definition, ("manifest",)))
    compare(at(native, ("input",)), at(manifest, ("cargo_identity",)))
    compare(
        at(native, ("mode",)),
        "actual-loader-cargo-identity-v1"
        if name == "loader-baseline"
        else "original-cargo-identity-api-v1",
    )
    for document in (native, control):
        compare(at(document, ("arms",)), 1)
        compare(at(document, ("consumptions",)), 1)
    compare(read_json(directory / "native/native-status.json"), "0")
    compare(
        (directory / "native/save/autosave/exit.sav").exists(),
        name == "loader-baseline",
    )
    bind_baseline(directory, job.oracle, native)
    compare(len(sequence(at(native, ("events",)))), total)
    included, excluded = indices(native, name)
    compare(len(included), included_count)
    compare(
        {"included": [*included], "excluded": [*excluded]}, at(expected, ("phases",))
    )
    observed = projection(native, name)
    mask = inherited_mask(native, name)
    for row in sequence(observed):
        compare(at(row, ("state", "standard_cargo_mask")), mask)
    compare(observed, read_json(directory / "rust.json"))
    compare(fingerprint(observed), at(expected, ("fingerprint",)))
    log = job.output / "logs" / log_name("native-" + name)
    compare(read_json(log / "argv.json") == native_argv(job, directory), rust=True)
    compare(at(read_json(log / "process.json"), ("returncode",)), 0)


def invocation_paths(output: Path) -> list[Json]:
    return [
        *sorted(
            str(path.relative_to(output))
            for path in output.rglob("invocation.txt")
            if not path.is_relative_to(output / "corruption")
        )
    ]


def validate(root: Path, output: Path, oracle: Path, layout: Json) -> None:
    sources(root, layout)
    verify_identity(root, output)
    cargo_binding(output)
    environment = mapping(read_json(output / "build-environment.json"))
    target = Path(text(at(environment, ("CARGO_TARGET_DIR",))))
    if (
        not target.is_absolute()
        or target.resolve().is_relative_to(root)
        or target.resolve().is_relative_to(output)
    ):
        raise WorldCheckError("Cargo identity target escaped external allocation")
    compare(
        environment,
        {
            "CARGO_TARGET_DIR": str(target),
            "CARGO_INCREMENTAL": "0",
            "CARGO_PROFILE_DEV_DEBUG": "0",
            "CARGO_PROFILE_TEST_DEBUG": "0",
            "CARGO_BUILD_JOBS": "1",
        },
    )
    binary = Path(
        text(at(read_json(output / "test-binaries.json"), ("ottd_sim", "retained")))
    )
    process(output / "logs" / log_name(SELECTOR), binary, SELECTOR)
    compare(
        at(
            read_json(output / "logs" / log_name(SELECTOR) / "environment.json"),
            ("OTTD_CARGO_IDENTITY_CASES",),
        ),
        str(output / "cases"),
    )
    for selector in FOCUSED:
        process(output / "logs" / log_name(selector), binary, selector, ignored=False)
    compare(read_json(output / "cases/summary.json"), list(CASES))
    compare(
        sorted(path.name for path in (output / "cases").iterdir() if path.is_dir())
        == sorted(CASES),
        rust=True,
    )
    compare(sorted(mapping(at(layout, ("cases",)))) == sorted(CASES), rust=True)
    definitions = sequence(read_json(root / COMPILER_INPUT))
    compare([at(row, ("name",)) for row in definitions], list(CASES))
    job = ControlRun(root, output, oracle)
    for definition, total, count in zip(
        definitions, EVENT_COUNTS, INCLUDED_COUNTS, strict=True
    ):
        case(
            job,
            definition,
            at(layout, ("cases", text(at(definition, ("name",))))),
            total,
            count,
        )
    invocations(root, output, oracle, at(layout, ("native_invocations",)))
