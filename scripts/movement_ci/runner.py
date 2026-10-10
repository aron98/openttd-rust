"""Complete four-case capture/admit orchestration; no subset corpus is accepted."""

from __future__ import annotations

import os
import shutil
import tempfile
from enum import StrEnum
from pathlib import Path
from typing import assert_never

from scripts.context_ci_support import zero
from scripts.grf_control_run import ControlRun

from . import (
    admission,
    archive_controls,
    artifacts,
    binding_controls,
    corruption,
    matrix,
    refusals,
)
from .baseline import FileInput
from .closure import sources, verify
from .host import size
from .native import Native, write
from .protocol import CASES
from .qualify import qualify
from .rust import build
from .value import DECODE, EvidenceError, Json, field, integer, require


class Mode(StrEnum):
    CAPTURE = "capture"
    ADMIT = "admit"


def run(root: Path, mode: Mode, requested: Path | None) -> None:
    forbidden = {key for key in os.environ if key.startswith("OTTD_")} - {
        "OTTD_GRF_ORACLE"
    }
    require("observer/subset environment forbidden", condition=not forbidden)
    layout = DECODE((root / "fixtures/road-movement/layout.json").read_text())
    require(
        "bounded capability layout",
        condition=field(layout, "capability")
        == "single-road-gameplay-physical-no-viewport",
    )
    cap = integer(field(layout, "proof_bytes"))
    floor = integer(field(layout, "free_floor"))
    output = destination(root, mode, requested, layout)
    require(
        "full capture launch reserve",
        condition=shutil.disk_usage(output).free >= floor + cap,
    )
    oracle = Path(
        os.environ.get(
            "OTTD_GRF_ORACLE", str(root / ".reference/snapshot-build/openttd")
        )
    ).resolve(strict=True)
    target = Path(
        os.environ.get(
            "CARGO_TARGET_DIR",
            str(Path(tempfile.gettempdir()) / ("movement-target-" + output.name)),
        )
    )
    require(
        "Cargo target boundary",
        condition=target.is_absolute()
        and not target.exists()
        and not target.is_relative_to(root)
        and not target.is_relative_to(output),
    )
    job = ControlRun(root, output, oracle)
    print(f"Artifacts: {output}", flush=True)
    closure = sources(root)
    verify(closure)
    artifacts.retain(output / "sources", closure)
    job.provenance()
    _ = job.run(
        "native-freshness",
        [
            "cmake",
            "-DORACLE=" + str(oracle),
            "-P",
            str(root / "scripts/check-replay-build.cmake"),
        ],
    )
    native_inputs = tuple(
        FileInput.capture(path)
        for path in sorted((root / ".reference/snapshot-source").rglob("*"))
        if path.is_file()
    )
    require("materialized actual native source required", condition=bool(native_inputs))
    config = FileInput.capture(root / "scripts/reference.cfg")
    executable = FileInput.capture(oracle)
    stamp = FileInput.capture(oracle.parent / "replay-build.sha256")
    artifacts.retain(
        output / "native-inputs", (*native_inputs, config, executable, stamp)
    )
    baseline = tuple(
        FileInput.capture(path)
        for path in sorted((oracle.parent / "baseset").rglob("*"))
        if path.is_file()
    )
    require("native baseline inventory required", condition=bool(baseline))
    native = Native(
        executable,
        config,
        (*closure, *native_inputs, stamp),
        baseline,
        output,
        cap,
        floor,
        integer(field(layout, "native_run_bytes")),
    )
    rust = build(job, target)
    for case in CASES:
        qualified = qualify(native, case, root / "fixtures/replay/clear-v362.sav")
        matrix.run(native, rust, qualified)
        refusals.run(rust, qualified)
        corruption.run(output, qualified)
        verify(closure)
        require(
            "aggregate proof cap",
            condition=size(output) < cap and shutil.disk_usage(output).free >= floor,
        )
    binding_controls.run(output, rust, native)
    zero(job, rust.harness.retained.path)
    admission.validate(output)
    rust.cli.verify()
    rust.harness.verify()
    native.verify()
    artifacts.retain(output / "baseline-inputs", baseline)
    write(
        output / "summary.json",
        {
            "schema_version": 1,
            "cases": [case.name for case in CASES],
            "capability": field(layout, "capability"),
            "rust_viewport": False,
            "status": "SCOPED_MATRIX_VERIFIED",
            "mode": mode.value,
        },
    )
    verify(closure)
    artifacts.seal(output)
    archive_controls.run(output)
    artifacts.seal(output)
    archive_controls.verify(output)
    require(
        "archived proof cap",
        condition=size(output) < cap and shutil.disk_usage(output).free >= floor,
    )
    print("PASS bounded road movement corpus", flush=True)


def destination(root: Path, mode: Mode, requested: Path | None, layout: Json) -> Path:
    match mode:
        case Mode.CAPTURE:
            require(
                "fresh external capture required",
                condition=requested is not None
                and requested.is_absolute()
                and not requested.exists()
                and not requested.is_relative_to(root),
            )
            if requested is None:
                raise EvidenceError("capture output required")
            output = requested
            output.mkdir(parents=True)
        case Mode.ADMIT:
            require(
                "normal CI awaits reviewed complete capture",
                condition=requested is None
                and field(layout, "verified_native_corpus") is True,
            )
            (root / ".artifacts").mkdir(exist_ok=True)
            output = Path(
                tempfile.mkdtemp(prefix="road-movement-", dir=root / ".artifacts")
            )
        case _:
            assert_never(mode)
    return output
