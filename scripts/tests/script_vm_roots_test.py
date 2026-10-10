"""Exact native session identity, retained cwd and corruption boundaries."""

import json
import shutil
from pathlib import Path

import pytest

from scripts import script_vm_roots as roots
from scripts.currency_ci_capture import source_names as currency_source_names
from scripts.engine_specs_ci_roster import GUARD_INPUT as ENGINE_GUARD_INPUT
from scripts.script_vm_provenance import digest
from scripts.world_check_support import ROOT, WorldCheckError, read_json


def prepare(directory: Path, name: str) -> Path:
    output = directory / "roots/sessions" / name
    output.mkdir(parents=True)
    _ = (output / "argv.json").write_text(json.dumps(roots.argv(directory, name)))
    _ = (output / "cwd.json").write_text(
        json.dumps(str(roots.working_directory(directory, name)))
    )
    _ = (output / "process.json").write_text('{"returncode":0,"expected":0}')
    _ = (output / "stderr.log").write_text("")
    _ = (output / "stdout.log").write_bytes(
        (ROOT / roots.SOURCE / roots.capture(name)).read_bytes()
    )
    roots.validate_session(directory, directory, name)
    return output


def test_real_source_corpus_has_exact_membership_and_shared_captures() -> None:
    assert len(roots.cases()) == 90
    assert len(roots.input_names()) == 122
    roots.validate_corpus()


def test_retained_actual_native_release_capture_is_accepted(tmp_path: Path) -> None:
    output = prepare(tmp_path, "ownership-compile-temp-buffer")
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
    output = prepare(tmp_path, "ownership-compile-temp-buffer")
    _ = (output / field).write_text(replacement)
    with pytest.raises(WorldCheckError):
        roots.validate_session(tmp_path, tmp_path, "ownership-compile-temp-buffer")


@pytest.mark.parametrize(("name", "before", "after"), list(roots.CONTROLS.values()))
def test_native_semantic_corruption_is_rejected(
    tmp_path: Path, name: str, before: str, after: str
) -> None:
    output = prepare(tmp_path, name)
    actual = (output / "stdout.log").read_text()
    assert before in actual
    _ = (output / "stdout.log").write_text(actual.replace(before, after))
    with pytest.raises(roots.RootObservationMismatchError):
        roots.validate_session(tmp_path, tmp_path, name)


@pytest.mark.parametrize("corruption", ["message", "status", "stderr"])
def test_controls_require_real_specific_admission_refusal(
    tmp_path: Path, corruption: str
) -> None:
    for session, _, _ in roots.CONTROLS.values():
        _ = prepare(tmp_path, session)
    roots.run_controls(tmp_path)
    roots.validate_controls(tmp_path, tmp_path)
    refused = tmp_path / "controls/root-write/admit-mutant"
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
        roots.validate_controls(tmp_path, tmp_path)


@pytest.mark.parametrize("mutation", ["capture", "hash", "session", "path"])
def test_pinned_corpus_member_or_manifest_mutation_is_refused(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, mutation: str
) -> None:
    _ = shutil.copytree(ROOT / roots.SOURCE, tmp_path / roots.SOURCE)
    _ = shutil.copytree(
        ROOT / "crates/ottd-script/tests/roots",
        tmp_path / "crates/ottd-script/tests/roots",
    )
    monkeypatch.setattr(roots, "ROOT", tmp_path)
    roots.validate_corpus()
    manifest = tmp_path / roots.SOURCE / "witnesses.json"
    data = read_json(manifest)
    assert isinstance(data, dict)
    files = data["files"]
    cases = data["cases"]
    assert isinstance(files, dict)
    assert isinstance(cases, list)
    match mutation:
        case "capture":
            member = tmp_path / roots.SOURCE / "native/read-buffer.txt"
            before = member.read_text()
            assert "integer 7" in before
            _ = member.write_text(before.replace("integer 7", "integer 8"))
        case "hash":
            files["native/read-buffer.txt"] = "0" * 64
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
        roots.validate_corpus()


def test_normal_manifest_retains_every_root_source_and_capture() -> None:
    manifest = read_json(ROOT / "scripts/script-vm-manifest.json")
    assert isinstance(manifest, dict)
    sources = manifest["sources"]
    paths = manifest["paths"]
    assert isinstance(sources, dict)
    assert isinstance(paths, list)
    for folder in [ROOT / roots.SOURCE, ROOT / "crates/ottd-script/tests/roots"]:
        for path in folder.rglob("*"):
            if path.is_file():
                name = str(path.relative_to(ROOT))
                assert sources[name] == digest(path)
                assert f"source/{name}" in paths
    assert "native/root_slots.hpp" in paths
    for name in roots.cases():
        assert f"roots/inputs/{roots.capture(name)}" in paths
        for filename in roots.SESSION_FILES:
            assert f"roots/sessions/{name}/{filename}" in paths


def test_engine_source_membership_matches_actual_capture_selector() -> None:
    layout = read_json(ROOT / "scripts/engine-specs-ci-layout.json")
    assert isinstance(layout, dict)
    sources = layout["sources"]
    assert isinstance(sources, dict)
    assert set(sources) == set(currency_source_names(ROOT)) | {ENGINE_GUARD_INPUT}
