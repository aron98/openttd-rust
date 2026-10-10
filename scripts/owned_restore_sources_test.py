from pathlib import Path

import pytest

from scripts.grf_control_evidence import sequence, text
from scripts.language_ci_compare import mapping
from scripts.owned_restore_sources import compiled_inputs, verify_compiled
from scripts.world_check_support import WorldCheckError, at, read_json


def fixture(root: Path) -> None:
    for name in ("ottd-core", "ottd-save", "ottd-sim", "ottd-cli"):
        path = root / "crates" / name / "src"
        path.mkdir(parents=True)
        _ = (path / "lib.rs").write_text("")
        _ = (path.parent / "Cargo.toml").write_text("[package]\n")
    _ = (root / "Cargo.toml").write_text("[workspace]\n")
    _ = (root / "Cargo.lock").write_text("version = 4\n")


def test_nested_embedded_input_required(tmp_path: Path) -> None:
    fixture(tmp_path)
    nested = tmp_path / "crates/ottd-sim/src/nested"
    nested.mkdir()
    _ = (tmp_path / "payload.bin").write_bytes(b"native fixture")
    _ = (nested / "test.rs").write_text(
        'const INPUT: &[u8] = include_bytes!("../../../../payload.bin");'
    )
    names = compiled_inputs(tmp_path)
    assert "payload.bin" in names
    assert "crates/ottd-sim/src/nested/test.rs" in names
    with pytest.raises(WorldCheckError, match="omits compiled"):
        verify_compiled(
            tmp_path, {name: "unused" for name in names if name != "payload.bin"}
        )


def test_dynamic_include_refused(tmp_path: Path) -> None:
    fixture(tmp_path)
    _ = (tmp_path / "crates/ottd-sim/src/lib.rs").write_text(
        'include_bytes!(concat!(env!("OUT_DIR"), "/generated"));'
    )
    with pytest.raises(WorldCheckError, match="Unresolved embedded"):
        _ = compiled_inputs(tmp_path)


def test_escaped_include_refused(tmp_path: Path) -> None:
    fixture(tmp_path)
    _ = (tmp_path / "crates/ottd-sim/src/lib.rs").write_text(
        'include_str!("../../../../outside");'
    )
    with pytest.raises(WorldCheckError, match="Missing or escaped"):
        _ = compiled_inputs(tmp_path)


def test_symlink_input_refused(tmp_path: Path) -> None:
    fixture(tmp_path)
    _ = (tmp_path / "payload.bin").write_bytes(b"fixture")
    (tmp_path / "alias.bin").symlink_to(tmp_path / "payload.bin")
    _ = (tmp_path / "crates/ottd-sim/src/lib.rs").write_text(
        'include_bytes!("../../../alias.bin");'
    )
    with pytest.raises(WorldCheckError, match="Missing or escaped"):
        _ = compiled_inputs(tmp_path)


def test_uncompiled_workspace_manifest_still_pinned(tmp_path: Path) -> None:
    fixture(tmp_path)
    other = tmp_path / "crates/ottd-script"
    other.mkdir()
    _ = (other / "Cargo.toml").write_text("[package]\n")
    assert "crates/ottd-script/Cargo.toml" in compiled_inputs(tmp_path)


def test_repository_wrong_source_copy_roster_matches_snapshot() -> None:
    root = Path(__file__).resolve().parents[1]
    layout = read_json(root / "scripts/owned-restore-layout.json")
    prefix = "admission/wrong-source/"
    expected = sorted(prefix + name for name in mapping(at(layout, ("sources",))))
    copied = sorted(
        text(path)
        for path in sequence(at(layout, ("control_paths",)))
        if text(path).startswith(prefix)
    )
    assert copied == expected
