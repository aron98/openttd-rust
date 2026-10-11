# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 -m scripts.rail_sources ROOT OUTPUT
"""Pinned source bytes and Git file modes at the rail build/run producer boundaries."""

from __future__ import annotations

import hashlib
import shutil
import sys
from pathlib import Path
from typing import Final

from scripts.owned_restore_sources import compiled_inputs, crate_inputs, regular
from scripts.world_check_support import Json, WorldCheckError, read_json, write_json

MANIFEST: Final = "scripts/rail-restore-source.json"
REQUIRED: Final = (
    "crates/ottd-sim/src/runtime.rs",
    "crates/ottd-sim/src/runtime/rail.rs",
    "crates/ottd-sim/src/runtime/rail/cache.rs",
    "crates/ottd-sim/src/runtime/rail/record.rs",
    "crates/ottd-sim/tests/native_rail_restoration.rs",
    "crates/ottd-sim/tests/native_rail_controls.rs",
    "crates/ottd-sim/tests/rail_restoration_support/mod.rs",
    "reference/rail_restore.hpp",
    "reference/rail_restore_fixture.hpp",
    "reference/rail_restore.patch",
    "scripts/build-rail-restore-reference.sh",
    "scripts/check-rail-restore-reference.cmake",
    "scripts/check-rail-restore.sh",
    "scripts/rail_sources.py",
    "scripts/tests/rail_sources_test.py",
    "scripts/owned_restore_sources.py",
    "scripts/world_check_support.py",
    "scripts/grf-control-ruff.toml",
    "scripts/run-reference.cmake",
    "scripts/reference.cfg",
    "fixtures/replay/clear-v362.sav",
    "upstream.toml",
    "rust-toolchain.toml",
)


def source_names(root: Path) -> tuple[str, ...]:
    root = root.resolve()
    names = set(
        crate_inputs(root, ("ottd-core", "ottd-save", "ottd-sim"), ("src", "tests"))
    )
    names.update(regular(root, root / name) for name in REQUIRED)
    names.update(compiled_inputs(root))
    return tuple(sorted(names))


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def git_mode(path: Path) -> str:
    return "100755" if path.stat().st_mode & 0o100 else "100644"


def mapping(value: Json) -> dict[str, Json]:
    match value:
        case dict() as entries:
            return entries
        case _:
            raise WorldCheckError("Rail source inventory requires an object")


def verify(root: Path) -> tuple[str, ...]:
    """Refuse missing or changed inputs before producer actions."""
    inventory = mapping(read_json(root / MANIFEST))
    hashes = mapping(inventory.get("sources"))
    modes = mapping(inventory.get("modes"))
    names = source_names(root)
    if set(hashes) != set(names) or set(modes) != set(names):
        raise WorldCheckError(
            "Rail source inventory membership differs from required inputs"
        )
    for name in names:
        path = root / name
        if digest(path) != hashes[name] or git_mode(path) != modes[name]:
            raise WorldCheckError(f"Rail source bytes/mode mismatch: {name}")
    return names


def capture(root: Path, output: Path) -> None:
    """Retain all verified producer inputs, then verify copied bytes and modes again."""
    names = verify(root)
    output.mkdir(parents=True, exist_ok=False)
    for name in names:
        target = output / "source" / name
        target.parent.mkdir(parents=True, exist_ok=True)
        _ = shutil.copy2(root / name, target)
    _ = shutil.copy2(root / MANIFEST, output / "inventory.json")
    verify_copies(root, output)
    write_json(
        output / "result.json",
        {"verified_inputs": len(names), "manifest_sha256": digest(root / MANIFEST)},
    )


def verify_copies(root: Path, output: Path) -> None:
    names = verify(root)
    if digest(output / "inventory.json") != digest(root / MANIFEST):
        raise WorldCheckError("Rail retained inventory mismatch")
    copied = (output / "source").resolve()
    physical = {
        path.relative_to(copied).as_posix()
        for path in copied.rglob("*")
        if path.is_file()
    }
    if physical != set(names):
        raise WorldCheckError("Rail retained source membership mismatch")
    for name in names:
        target = copied / name
        _ = regular(copied, target)
        if digest(target) != digest(root / name) or git_mode(target) != git_mode(
            root / name
        ):
            raise WorldCheckError(f"Rail retained source bytes/mode mismatch: {name}")


if __name__ == "__main__":
    project, destination = (Path(value).resolve() for value in sys.argv[1:])
    if destination.exists():
        verify_copies(project, destination)
    else:
        capture(project, destination)
