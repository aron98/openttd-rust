from __future__ import annotations

import os
import tempfile
from pathlib import Path
from typing import assert_never

from scripts.cargo_identity_ci_evidence import validate
from scripts.cargo_identity_ci_guards import native_argv, run_guards, validate_guards
from scripts.cargo_identity_ci_probes import live_probes
from scripts.cargo_identity_ci_roster import (
    CASES,
    COMPILER_INPUT,
    FOCUSED,
    LAYOUT,
    SELECTOR,
)
from scripts.context_ci_support import (
    Selection,
    build_lib,
    run_exact,
    sources,
    verify_identity,
    zero,
)
from scripts.depot_build_archive import package_raw
from scripts.engine_specs_ci_run import Mode, retain
from scripts.grf_control_evidence import sequence, text
from scripts.grf_control_run import ControlRun
from scripts.language_ci_compare import mapping
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


def prepare(root: Path, mode: Mode, requested: Path | None, layout: Json) -> Path:
    match mode:
        case Mode.CAPTURE:
            if (
                requested is None
                or not requested.is_absolute()
                or requested.exists()
                or requested.resolve().is_relative_to(root)
            ):
                raise WorldCheckError(
                    "Cargo capture requires fresh absolute external output"
                )
            requested.mkdir()
            return requested
        case Mode.ADMIT:
            if (
                requested is not None
                or at(layout, ("verified_native_corpus",)) is not True
            ):
                raise WorldCheckError(
                    "Cargo CI awaits complete paired corpus admission"
                )
            (root / ".artifacts").mkdir(exist_ok=True)
            return Path(
                tempfile.mkdtemp(prefix="cargo-identity-", dir=root / ".artifacts")
            )
        case _:
            assert_never(mode)


def run(mode: Mode, requested: Path | None = None) -> None:
    if any(
        name.startswith("OTTD_") and name != "OTTD_GRF_ORACLE" for name in os.environ
    ):
        raise WorldCheckError(
            "Cargo CI refuses inherited subset or observer environment"
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
            str(Path(tempfile.gettempdir()) / ("cargo-identity-target-" + output.name)),
        )
    )
    if (
        not target.is_absolute()
        or target.exists()
        or target.resolve().is_relative_to(root)
        or target.resolve().is_relative_to(output)
    ):
        raise WorldCheckError("Cargo identity CI requires fresh external Cargo target")
    variables = {
        "CARGO_TARGET_DIR": str(target),
        "CARGO_INCREMENTAL": "0",
        "CARGO_PROFILE_DEV_DEBUG": "0",
        "CARGO_PROFILE_TEST_DEBUG": "0",
        "CARGO_BUILD_JOBS": "1",
    }
    os.environ.update(variables)
    write_json(
        output / "build-environment.json",
        dict(variables.items()),
    )
    binary = build_lib(job)
    for definition in sequence(read_json(root / COMPILER_INPUT)):
        name = text(at(definition, ("name",)))
        directory = output / "cases" / name
        directory.mkdir(parents=True)
        write_json(directory / "manifest.json", at(definition, ("manifest",)))
        _ = job.run("native-" + name, native_argv(job, directory))
    write_json(output / "cases/summary.json", list(CASES))
    run_exact(
        job,
        binary,
        Selection(SELECTOR),
        {"OTTD_CARGO_IDENTITY_CASES": str(output / "cases")},
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
    write_json(
        output / "coverage.json",
        {
            "cases": list(CASES),
            "native_events": 250,
            "included_snapshots": 151,
            "excluded_snapshots": 99,
            "native_host_guards": 13,
            "wrapper_guards": 11,
            "admission_probes": 13,
            "focused": list(FOCUSED),
            "configured_world_admitted": False,
            "ambient_context_compared": False,
        },
    )
    match mode:
        case Mode.CAPTURE:
            write_json(
                output / "proposed-layout.json",
                {**mapping(layout), "verified_native_corpus": False},
            )
            message = "CAPTURE"
        case Mode.ADMIT:
            message = "PASS"
        case _:
            assert_never(mode)
    package_raw(output)
    summary = (
        message
        + " private raw cargo identity/translation/road corpus; "
        + "151 included, 99 excluded snapshots\n"
    )
    _ = (output / "summary.txt").write_text(summary)
    print(summary, end="", flush=True)
