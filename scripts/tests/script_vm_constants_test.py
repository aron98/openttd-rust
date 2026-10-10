"""Exact native session identity, retained cwd and corruption boundaries."""

import json
from pathlib import Path

import pytest

from scripts import script_vm_constants as constants
from scripts.world_check_support import ROOT, WorldCheckError


def prepare(directory: Path, name: str) -> Path:
    output = directory / "constants/sessions" / name
    output.mkdir(parents=True)
    _ = (output / "argv.json").write_text(json.dumps(constants.argv(directory, name)))
    _ = (output / "cwd.json").write_text(
        json.dumps(str(directory / "constants/inputs"))
    )
    _ = (output / "process.json").write_text('{"returncode":0,"expected":0}')
    _ = (output / "stderr.log").write_text("")
    _ = (output / "stdout.log").write_bytes(
        (ROOT / constants.SOURCE / "native" / f"{name}.txt").read_bytes()
    )
    constants.validate_session(directory, directory, name)
    return output


def test_real_source_corpus_has_exact_membership_and_shared_captures() -> None:
    assert len(constants.cases()) == 115
    assert len(constants.input_names()) == 152
    constants.validate_corpus()


def test_retained_actual_native_release_capture_is_accepted(tmp_path: Path) -> None:
    output = prepare(tmp_path, "release-child")
    assert (output / "stdout.log").is_file()


@pytest.mark.parametrize(
    ("field", "replacement"),
    [
        ("cwd.json", '"wrong-directory"'),
        ("argv.json", '["wrong-binary"]'),
        ("process.json", '{"returncode":-6,"expected":0}'),
        ("stderr.log", "observer_error"),
    ],
)
def test_session_identity_corruption_is_rejected(
    tmp_path: Path, field: str, replacement: str
) -> None:
    output = prepare(tmp_path, "release-child")
    _ = (output / field).write_text(replacement)
    with pytest.raises(WorldCheckError):
        constants.validate_session(tmp_path, tmp_path, "release-child")


@pytest.mark.parametrize(
    ("name", "before", "after"),
    [
        ("release-child", "string 11 ", "string 12 "),
        ("enum-counter-buffer", "lookup E D integer 1", "lookup E D integer 5"),
        ("replacement-failure", "lookup K - integer 7", "lookup K - absent"),
        ("release-child", "refs owned-value 0", "refs owned-value 1"),
    ],
)
def test_native_semantic_corruption_is_rejected(
    tmp_path: Path, name: str, before: str, after: str
) -> None:
    output = prepare(tmp_path, name)
    actual = (output / "stdout.log").read_text()
    assert before in actual
    _ = (output / "stdout.log").write_text(actual.replace(before, after))
    with pytest.raises(WorldCheckError):
        constants.validate_session(tmp_path, tmp_path, name)


@pytest.mark.parametrize("corruption", ["message", "status", "stderr"])
def test_controls_require_real_specific_admission_refusal(
    tmp_path: Path, corruption: str
) -> None:
    for session, _, _ in constants.CONTROLS.values():
        _ = prepare(tmp_path, session)
    constants.run_controls(tmp_path)
    constants.validate_controls(tmp_path, tmp_path)
    refused = tmp_path / "controls/constant-publication/admit-mutant"
    match corruption:
        case "message":
            _ = (refused / "stdout.log").write_text("unrelated rejection\n")
        case "status":
            _ = (refused / "process.json").write_text('{"returncode":2,"expected":1}')
        case "stderr":
            _ = (refused / "stderr.log").write_text("unrelated failure\n")
        case _:
            pytest.fail("unknown test corruption")
    with pytest.raises(WorldCheckError, match="admission receipt differs"):
        constants.validate_controls(tmp_path, tmp_path)
