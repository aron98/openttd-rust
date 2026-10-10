from __future__ import annotations

import os
import shutil
import tempfile
from enum import Enum
from pathlib import Path
from typing import assert_never

from scripts.context_ci_support import (
    Selection,
    build_lib,
    retain_native,
    run_exact,
    sources,
    verify_identity,
    zero,
)
from scripts.currency_ci_capture import source_names
from scripts.depot_build_archive import package_raw
from scripts.engine_specs_ci_evidence import validate
from scripts.engine_specs_ci_guards import native_argv, run_guards, validate_guards
from scripts.engine_specs_ci_probes import live_probes
from scripts.engine_specs_ci_roster import CASES, FOCUSED, GUARD_INPUT, LAYOUT, SELECTOR
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import sequence, text
from scripts.grf_control_run import ControlRun
from scripts.language_ci_compare import compare, mapping
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


class Mode(Enum):
    CAPTURE = "capture"
    ADMIT = "admit"


def prepare(root: Path, mode: Mode, requested: Path | None, layout: Json) -> Path:
    match mode:
        case Mode.CAPTURE:
            if (
                requested is None
                or not requested.is_absolute()
                or requested.exists()
                or requested.resolve().is_relative_to(root)
            ):
                raise WorldCheckError("Capture requires fresh absolute external output")
            output = requested
            output.mkdir()
        case Mode.ADMIT:
            if (
                requested is not None
                or at(layout, ("verified_native_corpus",)) is not True
            ):
                raise WorldCheckError(
                    "Engine CI awaits complete paired corpus admission"
                )
            (root / ".artifacts").mkdir(exist_ok=True)
            output = Path(
                tempfile.mkdtemp(prefix="engine-specs-", dir=root / ".artifacts")
            )
        case _:
            assert_never(mode)
    return output


def run(mode: Mode, requested: Path | None = None) -> None:
    forbidden = {
        "OTTD_ENGINE_SPECS_CASES",
        "OTTD_ENGINE_CONSTRUCTOR_PROOF",
        "OTTD_ENGINE_CASE",
        "OTTD_REPLAY_PATH",
        "OTTD_REPLAY_OUTPUT",
        "OTTD_GRF_CONTROL_MANIFEST",
        "OTTD_GRF_CONTROL_OUTPUT",
        "OTTD_ENGINE_SPECS_OUTPUT",
        "OTTD_GRF_CURRENCY_OUTPUT",
        "OTTD_GRF_STRINGS_OUTPUT",
        "OTTD_MOVEMENT_OBSERVE",
        "OTTD_MOVEMENT_PREPARE",
    }
    if forbidden.intersection(os.environ):
        raise WorldCheckError(
            "Engine CI refuses subset, replay or observer environment"
        )
    root = Path(__file__).resolve().parents[1]
    layout = read_json(root / LAYOUT)
    sources(root, layout)
    output = prepare(root, mode, requested, layout)
    print(f"Artifacts: {output}", flush=True)
    oracle = Path(
        os.environ.get(
            "OTTD_GRF_ORACLE", str(root / ".reference/snapshot-build/openttd")
        )
    ).resolve(strict=True)
    job = ControlRun(root, output, oracle)
    job.provenance()
    _ = job.run(
        "fresh-native",
        [
            "cmake",
            f"-DORACLE={oracle}",
            "-P",
            str(root / "scripts/check-replay-build.cmake"),
        ],
    )
    target = Path(
        os.environ.get(
            "CARGO_TARGET_DIR",
            str(Path(tempfile.gettempdir()) / ("engine-target-" + output.name)),
        )
    )
    if (
        not target.is_absolute()
        or target.exists()
        or target.resolve().is_relative_to(root)
        or target.resolve().is_relative_to(output)
    ):
        raise WorldCheckError("Engine CI requires fresh external Cargo target")
    os.environ["CARGO_TARGET_DIR"] = str(target)
    os.environ["CARGO_INCREMENTAL"] = "0"
    os.environ["CARGO_PROFILE_DEV_DEBUG"] = "0"
    os.environ["CARGO_PROFILE_TEST_DEBUG"] = "0"
    os.environ["CARGO_BUILD_JOBS"] = "1"
    write_json(
        output / "build-environment.json",
        {
            key: os.environ[key]
            for key in (
                "CARGO_TARGET_DIR",
                "CARGO_INCREMENTAL",
                "CARGO_PROFILE_DEV_DEBUG",
                "CARGO_PROFILE_TEST_DEBUG",
                "CARGO_BUILD_JOBS",
            )
        },
    )
    binary = build_lib(job)
    run_exact(
        job,
        binary,
        Selection(SELECTOR),
        {"OTTD_ENGINE_SPECS_CASES": str(output / "cases")},
    )
    for name in CASES:
        directory = output / "cases" / name
        _ = job.run(
            "native-" + name, native_argv(job, directory, directory / "manifest.json")
        )
    for selector in FOCUSED:
        run_exact(job, binary, Selection(selector, ignored=False), {})
    run_guards(job)
    validate(root, output, oracle, layout)
    live_probes(job, layout)
    validate(root, output, oracle, layout)
    validate_guards(job)
    zero(job, binary)
    verify_identity(root, output)
    retain(job)
    publish(job, layout, mode)


def retain(job: ControlRun) -> None:
    root, output, oracle = job.root, job.output, job.oracle
    retain_native(output, oracle)
    for name, sha in mapping(
        at(read_json(output / "provenance.json"), ("source_hashes",))
    ).items():
        destination = output / "source" / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        _ = shutil.copy2(root / name, destination)
        compare(digest(destination), sha)
    baseline = output / "baseline-sources"
    baseline.mkdir()
    for index, path in enumerate(
        sequence(
            at(
                read_json(output / "cases/loader-baseline/native/control.json"),
                ("baseline_sources",),
            )
        )
    ):
        source = Path(text(path))
        retained = baseline / f"{index}.grf"
        _ = shutil.copy2(source, retained)
        compare(digest(source), digest(retained))


def publish(job: ControlRun, layout: Json, mode: Mode) -> None:
    root, output = job.root, job.output
    write_json(
        output / "coverage.json",
        {
            "cases": list(CASES),
            "api_commands": 48,
            "loader_checkpoints": 6,
            "native_events": 280,
            "native_host_guards": 12,
            "wrapper_invocations": 9,
            "admission_probes": 13,
            "focused": list(FOCUSED),
        },
    )
    match mode:
        case Mode.CAPTURE:
            proposed: Json = {
                **mapping(layout),
                "verified_native_corpus": False,
                "sources": {
                    name: digest(root / name)
                    for name in (*source_names(root), GUARD_INPUT)
                },
            }
            write_json(output / "proposed-layout.json", proposed)
            message = "CAPTURE private raw engine/spec paired corpus; not CI admission"
        case Mode.ADMIT:
            message = "PASS private raw engine/spec paired corpus"
        case _:
            assert_never(mode)
    package_raw(output)
    _ = (output / "summary.txt").write_text(message + "\n")
    print(message, flush=True)
