"""Active observer source admission rejects missing, changed and mode-shifted inputs."""

import shutil
from pathlib import Path
from typing import Literal

import pytest

from scripts.script_vm_inputs import OBSERVER_INPUTS
from scripts.script_vm_provenance import verify_observer_sources
from scripts.world_check_support import WorldCheckError


@pytest.mark.parametrize("change", ["missing", "content", "mode", "symlink"])
def test_active_owner_observer_copy_is_mandatory(
    tmp_path: Path, change: Literal["missing", "content", "mode", "symlink"]
) -> None:
    root = Path(__file__).resolve().parents[2]
    native = tmp_path / "native"
    native.mkdir()
    for name in (*OBSERVER_INPUTS, "active_owners.hpp"):
        _ = shutil.copy2(root / "scripts/compat/script-vm" / name, native / name)
    verify_observer_sources(root, native)
    helper = native / "active_owners.hpp"
    original = helper.read_bytes()
    mode = helper.stat().st_mode
    match change:
        case "missing":
            helper.unlink()
        case "content":
            _ = helper.write_bytes(original + b"\n// changed compiled observer input\n")
        case "mode":
            helper.chmod(0o600)
        case "symlink":
            helper.unlink()
            helper.symlink_to(root / "scripts/compat/script-vm/active_owners.hpp")
    with pytest.raises(WorldCheckError, match="Native observer source differs"):
        verify_observer_sources(root, native)
    if helper.is_symlink():
        helper.unlink()
    _ = helper.write_bytes(original)
    helper.chmod(mode)
    verify_observer_sources(root, native)
