from pathlib import Path

import pytest

from scripts.currency_ci_capture import source_names
from scripts.engine_specs_ci_evidence import invocation_paths, projection
from scripts.engine_specs_ci_guards import native_argv, validate_guards
from scripts.engine_specs_ci_roster import CASES, COMPILER_INPUT, LAYOUT
from scripts.engine_specs_ci_run import Mode, prepare
from scripts.gameplay_foundations import log_name
from scripts.grf_control_evidence import sequence, text
from scripts.grf_control_run import ControlRun
from scripts.language_ci_compare import mapping
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


def test_normal_admission_refuses_unverified_layout(tmp_path: Path) -> None:
    with pytest.raises(WorldCheckError, match="awaits complete paired"):
        _ = prepare(tmp_path, Mode.ADMIT, None, {"verified_native_corpus": False})


def test_capture_does_not_reuse_or_enter_source(tmp_path: Path) -> None:
    root = tmp_path / "source"
    root.mkdir()
    for destination in (root / "new", tmp_path):
        with pytest.raises(WorldCheckError, match="fresh absolute external"):
            _ = prepare(root, Mode.CAPTURE, destination, {})
    result = prepare(root, Mode.CAPTURE, tmp_path / "capture", {})
    assert result.is_dir()


def test_engine_corpus_and_compiled_sources_are_complete() -> None:
    root = Path(__file__).resolve().parents[2]
    rows = sequence(at(read_json(root / COMPILER_INPUT), ("cases",)))
    assert [at(row, ("name",)) for row in rows] == list(CASES)
    assert sorted(mapping(at(read_json(root / LAYOUT), ("cases",)))) == sorted(CASES)
    names = source_names(root)
    assert COMPILER_INPUT in names
    assert "reference/engine_specs.hpp" in names
    assert "reference/engine_specs_hooks.hpp" in names
    assert "reference/engine_specs.patch" in names


def test_projection_rejects_an_unrecognized_state_field() -> None:
    native: Json = {
        "events": [{"phase": "api-command", "detail": None, "state": {"unexpected": 1}}]
    }
    with pytest.raises(WorldCheckError, match="state schema"):
        _ = projection(native, {"files": []}, "api-command")


def test_invocation_prefix_paths_use_serialized_order(tmp_path: Path) -> None:
    names = [
        "cases/loader-unknown/native/invocation.txt",
        "cases/loader-unknown-followup/native/invocation.txt",
        "guards/mixed-currency/native/invocation.txt",
        "corruption/copied/invocation.txt",
    ]
    for name in names:
        path = tmp_path / name
        path.parent.mkdir(parents=True)
        _ = path.write_text("retained invocation")
    assert invocation_paths(tmp_path) == [
        "cases/loader-unknown-followup/native/invocation.txt",
        "cases/loader-unknown/native/invocation.txt",
        "guards/mixed-currency/native/invocation.txt",
    ]


@pytest.fixture
def guard_job(tmp_path: Path) -> ControlRun:
    root = Path(__file__).resolve().parents[2]
    job = ControlRun(root, tmp_path, tmp_path / "oracle")
    rows = sequence(
        at(read_json(root / "scripts/engine-specs-ci-guards.json"), ("controls",))
    )
    definitions = {
        text(at(row, ("case",))): (
            at(row, ("manifest",)),
            text(at(row, ("expected_diagnostic",))),
        )
        for row in rows
    }
    for name, diagnostic in (
        ("existing", "run directory must be fresh"),
        ("manifest-budget", "manifest byte budget"),
        ("grf-budget", "configured-file byte budget"),
        ("save-hash", "requires exact clear-v362 fixture"),
    ):
        definitions[name] = ({}, diagnostic)
    variables = (
        "OTTD_REPLAY_PATH",
        "OTTD_GRF_CURRENCY_OUTPUT",
        "OTTD_GRF_STRINGS_OUTPUT",
        "OTTD_MOVEMENT_OBSERVE",
        "OTTD_MOVEMENT_PREPARE",
    )
    for variable in variables:
        definitions["mixed-" + variable.lower()] = ({}, "refuses mixed environment")
    summary: list[Json] = []
    for name, (manifest, diagnostic) in definitions.items():
        directory = tmp_path / "guards" / name
        directory.mkdir(parents=True)
        log = tmp_path / "logs" / log_name("guard-" + name)
        log.mkdir(parents=True)
        write_json(directory / "manifest.json", manifest)
        source = directory / "wrong.sav" if name == "save-hash" else None
        argv: list[Json] = [
            *native_argv(job, directory, directory / "manifest.json", source)
        ]
        write_json(log / "argv.json", argv)
        write_json(log / "process.json", {"returncode": 1})
        write_json(log / "environment.json", dict.fromkeys(variables, "1"))
        _ = (log / "stderr.log").write_text(diagnostic)
        if name == "existing":
            (directory / "native").mkdir()
            _ = (directory / "native/specs.json").write_text("sentinel\n")
        summary.append(
            {"name": name, "diagnostic": diagnostic, "existing": name == "existing"}
        )
    write_json(tmp_path / "guards/summary.json", summary)
    return job


def test_native_manifest_mixed_currency_is_not_wrapper_environment(
    guard_job: ControlRun,
) -> None:
    validate_guards(guard_job)


@pytest.mark.parametrize(
    "variable", ["OTTD_GRF_CURRENCY_OUTPUT", "OTTD_MOVEMENT_OBSERVE"]
)
def test_wrapper_environment_value_is_required(
    guard_job: ControlRun, variable: str
) -> None:
    validate_guards(guard_job)
    log = guard_job.output / "logs" / log_name("guard-mixed-" + variable.lower())
    write_json(log / "environment.json", {variable: "0"})
    with pytest.raises(WorldCheckError):
        validate_guards(guard_job)


@pytest.mark.parametrize("field", ["manifest", "argv", "exit", "diagnostic", "output"])
def test_native_mixed_guard_rejects_corrupted_evidence(
    guard_job: ControlRun, field: str
) -> None:
    validate_guards(guard_job)
    directory = guard_job.output / "guards/mixed-currency"
    log = guard_job.output / "logs" / log_name("guard-mixed-currency")
    mutations = {
        "manifest": directory / "manifest.json",
        "argv": log / "argv.json",
        "exit": log / "process.json",
        "diagnostic": log / "stderr.log",
        "output": directory / "native/specs.json",
    }
    target = mutations[field]
    target.parent.mkdir(parents=True, exist_ok=True)
    _ = target.write_text('{"returncode": 0}' if field == "exit" else "{}")
    with pytest.raises(WorldCheckError):
        validate_guards(guard_job)
