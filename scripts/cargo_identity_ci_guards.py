from __future__ import annotations

from pathlib import Path

from scripts.cargo_identity_ci_roster import GUARD_INPUT
from scripts.gameplay_foundations import log_name
from scripts.grf_control_evidence import sequence, text
from scripts.grf_control_run import ControlRun
from scripts.language_ci_compare import compare, mapping
from scripts.world_check_support import WorldCheckError, at, read_json, write_json


def native_argv(
    job: ControlRun, directory: Path, *, missing: bool = False, wrong: bool = False
) -> list[str]:
    argv = [
        "cmake",
        f"-DORACLE={job.oracle}",
        f"-DRUN_DIR={directory / 'native'}",
        f"-DCONFIG={job.root / 'scripts/reference.cfg'}",
    ]
    if not missing:
        source = (
            directory / "wrong.sav"
            if wrong
            else job.root / "fixtures/replay/clear-v362.sav"
        )
        argv.append(f"-DINPUT={source}")
    argv.extend(
        [
            f"-DMANIFEST={directory / 'manifest.json'}",
            "-P",
            str(job.root / "scripts/check-cargo-identity-reference.cmake"),
        ]
    )
    return argv


def run_guards(job: ControlRun) -> None:
    definitions = sequence(read_json(job.root / GUARD_INPUT))
    for row in definitions:
        name = text(at(row, ("name",)))
        directory = job.output / "guards" / name
        directory.mkdir(parents=True)
        write_json(directory / "manifest.json", at(row, ("manifest",)))
        if name == "wrapper-manifest-byte-budget":
            _ = (directory / "manifest.json").write_bytes(b" " * 65537)
        if name == "wrapper-existing-output-directory":
            (directory / "native").mkdir()
        if name == "wrapper-wrong-save-hash":
            _ = (directory / "wrong.sav").write_bytes(b"wrong")
        _ = job.run(
            "guard-" + name,
            native_argv(
                job,
                directory,
                missing=name == "wrapper-missing-required-argument",
                wrong=name == "wrapper-wrong-save-hash",
            ),
            {
                key: text(value)
                for key, value in mapping(at(row, ("environment",))).items()
            },
            expected=1,
        )
    write_json(
        job.output / "guards/summary.json", [at(row, ("name",)) for row in definitions]
    )
    validate_guards(job)


def validate_guards(job: ControlRun) -> None:
    definitions = sequence(read_json(job.root / GUARD_INPUT))
    compare(len(definitions), 24)
    compare(
        read_json(job.output / "guards/summary.json"),
        [at(row, ("name",)) for row in definitions],
    )
    compare(
        sorted(path.name for path in (job.output / "guards").iterdir() if path.is_dir())
        == sorted(text(at(row, ("name",))) for row in definitions),
        rust=True,
    )
    for row in definitions:
        name = text(at(row, ("name",)))
        directory = job.output / "guards" / name
        log = job.output / "logs" / log_name("guard-" + name)
        compare(
            read_json(log / "argv.json")
            == native_argv(
                job,
                directory,
                missing=name == "wrapper-missing-required-argument",
                wrong=name == "wrapper-wrong-save-hash",
            ),
            rust=True,
        )
        compare(at(read_json(log / "process.json"), ("returncode",)), 1)
        if name == "wrapper-manifest-byte-budget":
            compare(
                (directory / "manifest.json").read_bytes() == b" " * 65537, rust=True
            )
        else:
            compare(read_json(directory / "manifest.json"), at(row, ("manifest",)))
        environment = mapping(read_json(log / "environment.json"))
        compare(
            {
                key: value
                for key, value in environment.items()
                if key.startswith("OTTD_") and key != "OTTD_GRF_ORACLE"
            },
            at(row, ("environment",)),
        )
        errors = (log / "stderr.log").read_text()
        native_errors = directory / "native/stderr.log"
        if native_errors.exists():
            errors += native_errors.read_text()
        if text(at(row, ("diagnostic",))) not in errors:
            raise WorldCheckError("Cargo guard lacks expected actual diagnostic")
        if (directory / "native/cargo.json").exists():
            raise WorldCheckError("Cargo guard emitted an observation")
