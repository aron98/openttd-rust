"""Exact native session identity, retained cwd and corruption boundaries."""

import json
import shutil
from pathlib import Path

import pytest

from scripts import script_vm_arrays as arrays
from scripts.script_vm_provenance import digest
from scripts.world_check_support import ROOT, WorldCheckError, read_json


def prepare(directory: Path, name: str) -> Path:
    output = directory / "arrays/sessions" / name
    output.mkdir(parents=True)
    _ = (output / "argv.json").write_text(json.dumps(arrays.argv(directory, name)))
    _ = (output / "cwd.json").write_text(
        json.dumps(str(arrays.working_directory(directory, name)))
    )
    row = arrays.declarations()[name]
    _ = (output / "process.json").write_text(
        json.dumps({"returncode": row.status, "expected": row.status})
    )
    _ = (output / "stderr.log").write_text(row.stderr)
    _ = (output / "stdout.log").write_bytes(
        (ROOT / arrays.SOURCE / arrays.capture(name)).read_bytes()
    )
    arrays.validate_session(directory, directory, name)
    return output


def test_real_source_corpus_has_exact_membership_and_shared_captures() -> None:
    assert len(arrays.cases()) == 93
    assert len(arrays.input_names()) == 133
    arrays.validate_corpus()


def test_retained_actual_native_release_capture_is_accepted(tmp_path: Path) -> None:
    output = prepare(tmp_path, "attempt-01-child-temporary-buffer")
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
    output = prepare(tmp_path, "attempt-01-child-temporary-buffer")
    _ = (output / field).write_text(replacement)
    with pytest.raises(WorldCheckError):
        arrays.validate_session(tmp_path, tmp_path, "attempt-01-child-temporary-buffer")


@pytest.mark.parametrize(("name", "before", "after"), list(arrays.CONTROLS.values()))
def test_native_semantic_corruption_is_rejected(
    tmp_path: Path, name: str, before: str, after: str
) -> None:
    output = prepare(tmp_path, name)
    actual = (output / "stdout.log").read_text()
    assert before in actual
    _ = (output / "stdout.log").write_text(actual.replace(before, after))
    with pytest.raises(arrays.ArrayObservationMismatchError):
        arrays.validate_session(tmp_path, tmp_path, name)


@pytest.mark.parametrize("corruption", ["message", "status", "stderr"])
def test_controls_require_real_specific_admission_refusal(
    tmp_path: Path, corruption: str
) -> None:
    for session in {row[0] for row in arrays.CONTROLS.values()}:
        _ = prepare(tmp_path, session)
    arrays.run_controls(tmp_path)
    arrays.validate_controls(tmp_path, tmp_path)
    refused = tmp_path / "controls/array-construction/admit-mutant"
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
        arrays.validate_controls(tmp_path, tmp_path)


@pytest.mark.parametrize("mutation", ["capture", "hash", "session", "path"])
def test_pinned_corpus_member_or_manifest_mutation_is_refused(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, mutation: str
) -> None:
    _ = shutil.copytree(ROOT / arrays.SOURCE, tmp_path / arrays.SOURCE)
    _ = shutil.copytree(
        ROOT / "crates/ottd-script/tests/arrays-native",
        tmp_path / "crates/ottd-script/tests/arrays-native",
    )
    monkeypatch.setattr(arrays, "ROOT", tmp_path)
    arrays.validate_corpus()
    manifest = tmp_path / arrays.SOURCE / "witnesses.json"
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
                / arrays.SOURCE
                / "attempt-01/native/candidate-scores-buffer.txt"
            )
            before = member.read_text()
            assert "integer 1" in before
            _ = member.write_text(before.replace("integer 1", "integer 2"))
        case "hash":
            files["attempt-01/native/candidate-scores-buffer.txt"] = "0" * 64
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
        arrays.validate_corpus()


def test_normal_manifest_retains_every_array_source_and_capture() -> None:
    manifest = read_json(ROOT / "scripts/script-vm-manifest.json")
    assert isinstance(manifest, dict)
    sources = manifest["sources"]
    paths = manifest["paths"]
    assert isinstance(sources, dict)
    assert isinstance(paths, list)
    for folder in [
        ROOT / arrays.SOURCE,
        ROOT / "crates/ottd-script/tests/arrays-native",
    ]:
        for path in folder.rglob("*"):
            if path.is_file():
                name = str(path.relative_to(ROOT))
                assert sources[name] == digest(path)
                assert f"source/{name}" in paths
    assert "native/arrays.hpp" in paths
    for name in arrays.cases():
        assert f"arrays/inputs/{arrays.capture(name)}" in paths
        for filename in arrays.SESSION_FILES:
            assert f"arrays/sessions/{name}/{filename}" in paths


def test_protocol_refusals_are_not_success_statuses(tmp_path: Path) -> None:
    for name, row in arrays.declarations().items():
        if row.domain != "protocol-refusal":
            continue
        output = prepare(tmp_path, name)
        _ = (output / "process.json").write_text('{"returncode":0,"expected":0}')
        with pytest.raises(WorldCheckError, match="process failed"):
            arrays.validate_session(tmp_path, tmp_path, name)


def pointer_capture() -> tuple[str, str]:
    name = "attempt-01-ordering-conversion-buffer"
    return name, (ROOT / arrays.SOURCE / arrays.capture(name)).read_text()


def test_pointer_spelling_exclusion_preserves_native_format_and_identity() -> None:
    name, original = pointer_capture()
    fields = next(
        line.split()
        for line in original.splitlines()
        if line.startswith("return 9996 string ")
    )
    old = " ".join(fields[-2:])
    payload = b"(array : 0x12345678)"
    changed = original.replace(old, f"{len(payload)} {payload.hex()}")
    assert changed != original
    assert arrays.comparable(name, changed) == arrays.comparable(name, original)


@pytest.mark.parametrize("mutation", ["state", "prefix", "length", "alias", "missing"])
def test_pointer_exclusion_rejects_other_observation_changes(
    tmp_path: Path, mutation: str
) -> None:
    name, original = pointer_capture()
    output = prepare(tmp_path, name)
    record = next(
        line for line in original.splitlines() if line.startswith("return 9996 string ")
    )
    match mutation:
        case "state":
            changed = original.replace("op 31 2 0 0 0", "op 31 2 1 0 0")
        case "prefix":
            changed = original.replace("return 9996 string ", "return 9995 string ")
        case "length":
            fields = record.split()
            changed = original.replace(
                record, " ".join([*fields[:-2], "1", fields[-1]])
            )
        case "alias":
            payload = b"(array : 0x12345678)"
            changed = original.replace(
                record, f"return 9996 string {len(payload)} {payload.hex()}"
            )
        case "missing":
            changed = original.replace(record + "\n", "")
        case _:
            pytest.fail("unknown pointer mutation")
    _ = (output / "stdout.log").write_text(changed)
    with pytest.raises(arrays.ArrayObservationMismatchError):
        arrays.validate_session(tmp_path, tmp_path, name)
