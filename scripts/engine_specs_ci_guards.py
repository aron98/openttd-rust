from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from scripts.engine_specs_ci_roster import GUARD_INPUT
from scripts.gameplay_foundations import log_name
from scripts.grf_control_evidence import sequence, text
from scripts.grf_control_run import ControlRun
from scripts.language_ci_compare import compare
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


@dataclass(frozen=True, slots=True)
class Options:
    variable: str | None = None
    existing: bool = False
    padding: bool = False
    wrong_save: bool = False
    large_grf: bool = False


def native_argv(
    job: ControlRun, directory: Path, manifest: Path, source: Path | None = None
) -> list[str]:
    return [
        "cmake",
        f"-DORACLE={job.oracle}",
        f"-DRUN_DIR={directory / 'native'}",
        f"-DCONFIG={job.root / 'scripts/reference.cfg'}",
        f"-DINPUT={source or job.root / 'fixtures/replay/clear-v362.sav'}",
        f"-DMANIFEST={manifest}",
        "-P",
        str(job.root / "scripts/check-engine-specs-reference.cmake"),
    ]


def run_guards(job: ControlRun) -> None:
    directory = job.output / "guards"
    directory.mkdir()
    summary: list[Json] = []

    def invoke(
        name: str,
        manifest: Json,
        diagnostic: str,
        options: Options | None = None,
    ) -> None:
        options = options or Options()
        case = directory / name
        case.mkdir()
        if options.large_grf:
            path = case / "large.grf"
            _ = path.write_bytes(b"X" * 4097)
            manifest = {
                "networking": False,
                "engine_specs": {"mode": "load", "dynamic_engines": True},
                "files": [
                    {
                        "path": str(path),
                        "grfid": 1,
                        "metadata_version": 0,
                        "parameters": [],
                        "static": False,
                        "init_only": False,
                        "system": False,
                    }
                ],
            }
        write_json(case / "manifest.json", manifest)
        if options.padding:
            with (case / "manifest.json").open("a") as stream:
                _ = stream.write(" " * 65537)
        source = None
        if options.wrong_save:
            source = case / "wrong.sav"
            _ = source.write_bytes(b"wrong")
        if options.existing:
            (case / "native").mkdir()
            _ = (case / "native/specs.json").write_text("sentinel\n")
        _ = job.run(
            "guard-" + name,
            native_argv(job, case, case / "manifest.json", source),
            {options.variable: "1"} if options.variable is not None else {},
            expected=1,
        )
        summary.append(
            {"name": name, "diagnostic": diagnostic, "existing": options.existing}
        )

    for row in sequence(at(read_json(job.root / GUARD_INPUT), ("controls",))):
        invoke(
            text(at(row, ("case",))),
            at(row, ("manifest",)),
            text(at(row, ("expected_diagnostic",))),
        )
    baseline: Json = {
        "networking": False,
        "files": [],
        "engine_specs": {"mode": "load", "dynamic_engines": True},
    }
    invoke("existing", baseline, "run directory must be fresh", Options(existing=True))
    invoke("manifest-budget", baseline, "manifest byte budget", Options(padding=True))
    invoke(
        "grf-budget", baseline, "configured-file byte budget", Options(large_grf=True)
    )
    invoke(
        "save-hash",
        baseline,
        "requires exact clear-v362 fixture",
        Options(wrong_save=True),
    )
    for variable in (
        "OTTD_REPLAY_PATH",
        "OTTD_GRF_CURRENCY_OUTPUT",
        "OTTD_GRF_STRINGS_OUTPUT",
        "OTTD_MOVEMENT_OBSERVE",
        "OTTD_MOVEMENT_PREPARE",
    ):
        invoke(
            "mixed-" + variable.lower(),
            baseline,
            "refuses mixed environment",
            options=Options(variable=variable),
        )
    write_json(directory / "summary.json", summary)
    validate_guards(job)


def validate_guards(job: ControlRun) -> None:
    definitions = {
        text(at(row, ("case",))): at(row, ("manifest",))
        for row in sequence(at(read_json(job.root / GUARD_INPUT), ("controls",)))
    }
    expected = [
        (text(at(row, ("case",))), text(at(row, ("expected_diagnostic",))))
        for row in sequence(at(read_json(job.root / GUARD_INPUT), ("controls",)))
    ]
    expected.extend(
        [
            ("existing", "run directory must be fresh"),
            ("manifest-budget", "manifest byte budget"),
            ("grf-budget", "configured-file byte budget"),
            ("save-hash", "requires exact clear-v362 fixture"),
        ]
    )
    expected.extend(
        ("mixed-" + variable.lower(), "refuses mixed environment")
        for variable in (
            "OTTD_REPLAY_PATH",
            "OTTD_GRF_CURRENCY_OUTPUT",
            "OTTD_GRF_STRINGS_OUTPUT",
            "OTTD_MOVEMENT_OBSERVE",
            "OTTD_MOVEMENT_PREPARE",
        )
    )
    summary = read_json(job.output / "guards/summary.json")
    compare(
        summary,
        [
            {"name": name, "diagnostic": diagnostic, "existing": name == "existing"}
            for name, diagnostic in expected
        ],
    )
    compare(
        sorted(p.name for p in (job.output / "guards").iterdir() if p.is_dir())
        == sorted(name for name, _ in expected),
        rust=True,
    )
    for name, diagnostic in expected:
        directory = job.output / "guards" / name
        log = job.output / "logs" / log_name("guard-" + name)
        source = directory / "wrong.sav" if name == "save-hash" else None
        compare(
            read_json(log / "argv.json")
            == native_argv(job, directory, directory / "manifest.json", source),
            rust=True,
        )
        if name in definitions:
            compare(read_json(directory / "manifest.json"), definitions[name])
        if name.startswith("mixed-ottd_"):
            variable = name.removeprefix("mixed-").upper()
            compare(at(read_json(log / "environment.json"), (variable,)), "1")
        compare(at(read_json(log / "process.json"), ("returncode",)), 1)
        errors = (log / "stderr.log").read_text()
        native_errors = directory / "native/stderr.log"
        if native_errors.exists():
            errors += native_errors.read_text()
        if diagnostic not in errors:
            raise WorldCheckError("Engine host guard lacks expected native diagnostic")
        output = directory / "native/specs.json"
        if name == "existing":
            compare(output.read_text(), "sentinel\n")
        elif output.exists():
            raise WorldCheckError("Engine host guard emitted an observation")
