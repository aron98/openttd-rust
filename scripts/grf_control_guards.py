from __future__ import annotations

import shutil
from pathlib import Path

from scripts.gameplay_foundations import digest, require_test
from scripts.grf_control_run import ControlRun
from scripts.world_check_support import WorldCheckError, write_json


def guards(run: ControlRun, binary: Path) -> None:
    base = run.output / "guards"
    base.mkdir()
    manifest = run.output / "results/op-00-ordinary/manifest.json"
    save = str(run.root / "fixtures/replay/clear-v362.sav")
    runner = run.root / "scripts/check-grf-load-control-reference.cmake"

    def command(destination: Path, oracle: Path, source: str = save) -> list[str]:
        return [
            "cmake",
            f"-DORACLE={oracle}",
            f"-DRUN_DIR={destination}",
            f"-DCONFIG={run.root / 'scripts/reference.cfg'}",
            f"-DINPUT={source}",
            f"-DMANIFEST={manifest}",
            "-P",
            str(runner),
        ]

    stale = base / "stale-binary"
    stale.mkdir()
    _ = shutil.copy2(run.oracle, stale / "openttd")
    _ = (stale / "replay-build.sha256").write_text("deliberately stale build stamp\n")
    for name, source, oracle, diagnostic in (
        ("stale", save, stale / "openttd", "Stale native replay binary/source"),
        ("replay", save, run.oracle, "requires a dedicated invocation"),
        ("non-save", "GENERATE", run.oracle, "requires an explicit saved-game load"),
        ("reused", save, run.oracle, "run directory already exists"),
    ):
        destination = base / name
        if name == "reused":
            destination.mkdir()
        result = run.run(
            f"guard-{name}",
            command(destination, oracle, source),
            {"OTTD_REPLAY_PATH": "forbidden"} if name == "replay" else None,
            expected=1,
        )
        if diagnostic not in result.stderr or (
            name != "reused" and destination.exists()
        ):
            raise WorldCheckError(
                f"Loader guard did not reject before invocation: {name}"
            )

    unarmed = base / "unarmed.json"
    argv = command(base / "unarmed", run.oracle, "GENERATE")
    argv[-1] = str(run.root / "scripts/run-reference.cmake")
    argv.insert(-2, "-DTICKS=1")
    variables = {
        "OTTD_GRF_CONTROL_MANIFEST": str(manifest),
        "OTTD_GRF_CONTROL_OUTPUT": str(unarmed),
    }
    _ = run.run("guard-unarmed", argv, variables)
    if unarmed.exists() or not (base / "unarmed/save/autosave/exit.sav").is_file():
        raise WorldCheckError("Non-target startup or generated load armed observer")

    existing = run.output / "results/op-00-ordinary/native/control.json"
    before = digest(existing)
    argv = command(base / "duplicate", run.oracle)
    argv[-1] = str(run.root / "scripts/run-reference.cmake")
    argv.insert(-2, "-DTICKS=1")
    variables["OTTD_GRF_CONTROL_OUTPUT"] = str(existing)
    result = run.run("guard-duplicate", argv, variables, expected=1)
    diagnostic = "GRF control observer host refusal: control output already exists"
    if (
        "Reference engine failed (1)" not in result.stderr
        or diagnostic not in (base / "duplicate/stderr.log").read_text()
        or digest(existing) != before
    ):
        raise WorldCheckError(
            "Native duplicate-output guard did not preserve original evidence"
        )

    dispatcher_guards(run, binary)
    write_json(
        base / "assertions.json",
        {
            "stale": True,
            "replay": True,
            "reused": True,
            "non_save": True,
            "unarmed": True,
            "duplicate_preserved_sha256": before,
            "zero_test": True,
            "case_subset": True,
        },
    )


def dispatcher_guards(run: ControlRun, binary: Path) -> None:
    zero = run.run(
        "guard-zero-test",
        [str(binary), "--exact", "nonexistent_loader_test", "--ignored"],
    )
    try:
        require_test(zero.stdout, "native_load_control_matrix")
    except WorldCheckError:
        if "0 passed; 0 failed;" not in zero.stdout:
            raise WorldCheckError("Zero-test control was not genuine") from None
    else:
        raise WorldCheckError("Zero-test dispatcher was accepted")
    subset = run.run(
        "guard-subset",
        ["python3", "scripts/check-grf-control.py"],
        {"OTTD_GRF_CONTROL_CASE": "op-00-ordinary"},
        expected=1,
    )
    if "refuses OTTD_GRF_CONTROL_CASE" not in subset.stderr:
        raise WorldCheckError("Case-subset driver was accepted")
