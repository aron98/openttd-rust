from __future__ import annotations

import os
import tempfile
from enum import Enum
from pathlib import Path
from typing import assert_never

from scripts.context_ci_support import (
    Selection,
    build_lib,
    process,
    run_exact,
    sources,
    zero,
)
from scripts.currency_ci_bindings import compiler_inputs
from scripts.currency_properties_ci_archive import retain
from scripts.currency_properties_ci_capture import capture
from scripts.currency_properties_ci_evidence import validate
from scripts.currency_properties_ci_guards import run_guards
from scripts.currency_properties_ci_probes import live_probes
from scripts.currency_properties_ci_roster import (
    COMPILER_INPUT,
    FOCUSED,
    GUARD_INPUT,
    SELECTORS,
)
from scripts.depot_build_archive import package_raw
from scripts.gameplay_foundations import digest, log_name
from scripts.grf_control_run import ControlRun
from scripts.language_ci_compare import compare, mapping
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


class Mode(Enum):
    ADMIT = "admit"
    CAPTURE = "capture"


def prepare_directory(root: Path, mode: Mode, requested: Path | None) -> Path:
    match mode:
        case Mode.ADMIT:
            if requested is not None:
                raise WorldCheckError("Normal admission owns its artifact directory")
            parent = root / ".artifacts"
            parent.mkdir(exist_ok=True)
            return Path(tempfile.mkdtemp(prefix="grf-currency-properties-", dir=parent))
        case Mode.CAPTURE:
            if (
                requested is None
                or not requested.is_absolute()
                or requested.resolve().is_relative_to(root.resolve())
            ):
                raise WorldCheckError(
                    "Capture output must be absolute and outside source"
                )
            if requested.exists():
                raise WorldCheckError("Capture output must be fresh")
            requested.mkdir(exist_ok=False)
            return requested
        case _:
            assert_never(mode)


def fresh_build(job: ControlRun) -> Path:
    previous = os.environ.get("CARGO_TARGET_DIR")
    target = (
        Path(previous)
        if previous
        else Path(tempfile.gettempdir())
        / ("openttd-currency-target-" + job.output.name)
    )
    if (
        not target.is_absolute()
        or target.exists()
        or target.resolve().is_relative_to(job.output.resolve())
        or target.resolve().is_relative_to(job.root.resolve())
    ):
        raise WorldCheckError(
            "Currency producer requires a fresh absolute external target"
        )
    try:
        os.environ["CARGO_TARGET_DIR"] = str(target)
        return build_lib(job)
    finally:
        if previous is None:
            del os.environ["CARGO_TARGET_DIR"]
        else:
            os.environ["CARGO_TARGET_DIR"] = previous


def execute(job: ControlRun) -> Path:
    job.provenance()
    compiler_inputs(job.root, job.output)
    provenance = read_json(job.output / "provenance.json")
    hashes = mapping(at(provenance, ("source_hashes",)))
    for name in (COMPILER_INPUT, GUARD_INPUT):
        hashes[name] = digest(job.root / name)
    write_json(job.output / "provenance.json", provenance)
    binary = fresh_build(job)
    for group, selector in SELECTORS.items():
        run_exact(
            job,
            binary,
            Selection(
                selector,
                actual=os.environ.get("OTTD_CURRENCY_PROPERTIES_TEST_SELECTOR"),
            ),
            {
                "OTTD_GRF_CURRENCY_ORACLE": str(job.oracle),
                "OTTD_GRF_CURRENCY_ARTIFACTS": str(job.output / group),
            },
        )
    for selector in FOCUSED:
        run_exact(job, binary, Selection(selector, ignored=False), {})
        process(
            job.output / "logs" / log_name(selector), binary, selector, ignored=False
        )
    run_guards(job.root, job.output, job.oracle)
    return binary


def publish(job: ControlRun, layout: Json, mode: Mode) -> None:
    write_json(job.output / "coverage.json", at(layout, ("totals",)))
    match mode:
        case Mode.CAPTURE:
            proposed = {**mapping(layout), "verified_native_corpus": False}
            write_json(job.output / "proposed-layout.json", proposed)
            message = "CAPTURE currency 0B-0F complete fixed corpus; not CI admission"
        case Mode.ADMIT:
            message = "PASS currency 0B-0F complete fixed corpus"
        case _:
            assert_never(mode)
    package_raw(job.output)
    _ = (job.output / "summary.txt").write_text(message + "\n")
    print(message, flush=True)


def admission_layout(root: Path) -> Json:
    layout = read_json(root / "scripts/currency-properties-ci-layout.json")
    if at(layout, ("verified_native_corpus",)) is not True:
        raise WorldCheckError("Currency property CI awaits complete original corpus")
    sources(root, layout)
    compare(at(layout, ("focused",)), list(FOCUSED))
    return layout


def run(mode: Mode, output: Path | None = None) -> None:
    forbidden = {
        "OTTD_GRF_CURRENCY_CASE",
        "OTTD_GRF_CONTROL_CASE",
        "OTTD_GRF_LANGUAGE_CASE",
        "OTTD_REPLAY_PATH",
        "OTTD_GRF_CONTROL_MANIFEST",
        "OTTD_GRF_CONTROL_OUTPUT",
        "OTTD_GRF_CURRENCY_OUTPUT",
        "OTTD_GRF_LANGUAGE_OUTPUT",
    }
    if forbidden.intersection(os.environ):
        raise WorldCheckError(
            "Currency property producer refuses subset/replay/observer environment"
        )
    root = Path(__file__).resolve().parents[1]
    layout: Json = None
    match mode:
        case Mode.ADMIT:
            layout = admission_layout(root)
        case Mode.CAPTURE:
            pass
        case _:
            assert_never(mode)
    directory = prepare_directory(root, mode, output)
    print(f"Artifacts: {directory}", flush=True)
    oracle = Path(
        os.environ.get(
            "OTTD_GRF_ORACLE", str(root / ".reference/snapshot-build/openttd")
        )
    ).resolve(strict=True)
    job = ControlRun(root, directory, oracle)
    binary = execute(job)
    match mode:
        case Mode.CAPTURE:
            layout = capture(root, directory, oracle)
        case Mode.ADMIT:
            pass
        case _:
            assert_never(mode)
    validate(root, directory, oracle, layout)
    live_probes(root, directory, oracle, layout)
    zero(job, binary)
    retain(job, binary)
    publish(job, layout, mode)
