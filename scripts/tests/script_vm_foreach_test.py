"""Exact native session identity, retained cwd and corruption boundaries."""

import json
import shutil
from pathlib import Path

import pytest

from scripts import script_vm_foreach as foreach
from scripts.script_vm_provenance import digest, load_spec
from scripts.world_check_support import ROOT, WorldCheckError, read_json


def prepare(directory: Path, name: str) -> Path:
    output = directory / "foreach/sessions" / name
    output.mkdir(parents=True)
    _ = (output / "argv.json").write_text(json.dumps(foreach.argv(directory, name)))
    _ = (output / "cwd.json").write_text(
        json.dumps(str(foreach.working_directory(directory, name)))
    )
    row = foreach.declarations()[name]
    _ = (output / "process.json").write_text(
        json.dumps({"returncode": row.status, "expected": row.status})
    )
    _ = (output / "stderr.log").write_text(row.stderr)
    _ = (output / "stdout.log").write_bytes(
        (ROOT / foreach.SOURCE / foreach.capture(name)).read_bytes()
    )
    foreach.validate_session(directory, directory, name)
    return output


def test_real_source_corpus_has_exact_membership_and_shared_captures() -> None:
    assert len(foreach.cases()) == 135
    assert len(foreach.input_names()) == 181
    foreach.validate_corpus()


def test_retained_actual_native_release_capture_is_accepted(tmp_path: Path) -> None:
    output = prepare(tmp_path, "attempt-02-child-owner-0-buffer")
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
    output = prepare(tmp_path, "attempt-02-child-owner-0-buffer")
    _ = (output / field).write_text(replacement)
    with pytest.raises(WorldCheckError):
        foreach.validate_session(tmp_path, tmp_path, "attempt-02-child-owner-0-buffer")


@pytest.mark.parametrize(("name", "before", "after"), list(foreach.CONTROLS.values()))
def test_native_semantic_corruption_is_rejected(
    tmp_path: Path, name: str, before: str, after: str
) -> None:
    output = prepare(tmp_path, name)
    actual = (output / "stdout.log").read_text()
    assert before in actual
    _ = (output / "stdout.log").write_text(actual.replace(before, after))
    with pytest.raises(foreach.ForeachObservationMismatchError):
        foreach.validate_session(tmp_path, tmp_path, name)


@pytest.mark.parametrize("corruption", ["message", "status", "stderr"])
def test_controls_require_real_specific_admission_refusal(
    tmp_path: Path, corruption: str
) -> None:
    for session in {row[0] for row in foreach.CONTROLS.values()}:
        _ = prepare(tmp_path, session)
    foreach.run_controls(tmp_path)
    foreach.validate_controls(tmp_path, tmp_path)
    refused = tmp_path / "controls/foreach-cursor/admit-mutant"
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
        foreach.validate_controls(tmp_path, tmp_path)


@pytest.mark.parametrize("mutation", ["capture", "hash", "session", "path"])
def test_pinned_corpus_member_or_manifest_mutation_is_refused(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, mutation: str
) -> None:
    _ = shutil.copytree(ROOT / foreach.SOURCE, tmp_path / foreach.SOURCE)
    _ = shutil.copytree(
        ROOT / "crates/ottd-script/tests/foreach-native",
        tmp_path / "crates/ottd-script/tests/foreach-native",
    )
    monkeypatch.setattr(foreach, "ROOT", tmp_path)
    foreach.validate_corpus()
    manifest = tmp_path / foreach.SOURCE / "witnesses.json"
    data = read_json(manifest)
    assert isinstance(data, dict)
    files = data["files"]
    cases = data["cases"]
    assert isinstance(files, dict)
    assert isinstance(cases, list)
    match mutation:
        case "capture":
            member = (
                tmp_path
                / foreach.SOURCE
                / "attempt-02/native/future-host-write-0-buffer.txt"
            )
            before = member.read_text()
            assert "integer 1" in before
            _ = member.write_text(before.replace("integer 1", "integer 2"))
        case "hash":
            files["attempt-02/native/future-host-write-0-buffer.txt"] = "0" * 64
        case "session":
            _ = cases.pop()
        case "path":
            case = cases[0]
            assert isinstance(case, dict)
            case["session"] = "../escaped.session"
        case _:
            pytest.fail("unknown corpus mutation")
    _ = manifest.write_text(json.dumps(data))
    with pytest.raises(WorldCheckError):
        foreach.validate_corpus()


def test_normal_manifest_retains_every_foreach_source_and_capture() -> None:
    manifest = read_json(ROOT / "scripts/script-vm-manifest.json")
    assert isinstance(manifest, dict)
    sources = manifest["sources"]
    paths = manifest["paths"]
    assert isinstance(sources, dict)
    assert isinstance(paths, list)
    for folder in [
        ROOT / foreach.SOURCE,
        ROOT / "crates/ottd-script/tests/foreach-native",
    ]:
        for path in folder.rglob("*"):
            if path.is_file():
                name = str(path.relative_to(ROOT))
                assert sources[name] == digest(path)
                assert f"source/{name}" in paths
    assert "native/arrays.hpp" in paths
    for name in foreach.cases():
        assert f"foreach/inputs/{foreach.capture(name)}" in paths
        for filename in foreach.SESSION_FILES:
            assert f"foreach/sessions/{name}/{filename}" in paths


def test_actual_control_artifacts_match_declared_manifest(tmp_path: Path) -> None:
    for session in {row[0] for row in foreach.CONTROLS.values()}:
        _ = prepare(tmp_path, session)
    foreach.run_controls(tmp_path)
    actual = {
        str(path.relative_to(tmp_path))
        for path in (tmp_path / "controls").rglob("*")
        if path.is_file()
    }
    declared = {
        name
        for name in load_spec(ROOT / "scripts/script-vm-manifest.json").paths
        if name.startswith("controls/foreach-")
    }
    assert actual == declared
