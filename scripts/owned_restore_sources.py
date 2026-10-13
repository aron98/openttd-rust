from __future__ import annotations

import re
from pathlib import Path
from typing import Final

from scripts.world_check_support import Json, WorldCheckError

CRATES: Final = ("ottd-core", "ottd-save", "ottd-sim", "ottd-cli")
INCLUDE: Final = re.compile(r"\binclude(?:_bytes|_str)?!")
LITERAL: Final = re.compile(r'\binclude(?:_bytes|_str)?!\s*\(\s*"([^"\n]+)"\s*,?\s*\)')


def regular(root: Path, path: Path) -> str:
    resolved = path.resolve()
    if (
        not resolved.is_relative_to(root.resolve())
        or not path.is_file()
        or any(part.is_symlink() for part in (path, *path.parents) if part != root)
    ):
        raise WorldCheckError(f"Missing or escaped compiled input: {path}")
    return resolved.relative_to(root.resolve()).as_posix()


def compiled_inputs(root: Path) -> tuple[str, ...]:
    """Inventory the CLI dependency crates and simulator lib's nested test inputs."""
    return crate_inputs(root, CRATES)


def crate_inputs(
    root: Path, crates: tuple[str, ...], folders: tuple[str, ...] = ("src",)
) -> tuple[str, ...]:
    """Inventory selected Cargo targets and literal embedded source/data inputs."""
    required = {regular(root, root / name) for name in ("Cargo.toml", "Cargo.lock")}
    required.update(
        regular(root, path) for path in (root / "crates").glob("*/Cargo.toml")
    )
    required.update(regular(root, path) for path in (root / ".cargo").rglob("*.toml"))
    pending: list[Path] = []
    for name in crates:
        crate = root / "crates" / name
        required.add(regular(root, crate / "Cargo.toml"))
        for folder in folders:
            pending.extend(sorted((crate / folder).rglob("*.rs")))
        if (crate / "build.rs").exists():
            raise WorldCheckError(f"Unreviewed compiled build script: {name}")
    inspected: set[str] = set()
    while pending:
        source = pending.pop()
        name = regular(root, source)
        required.add(name)
        if name in inspected:
            continue
        inspected.add(name)
        body = source.read_text()
        literals = list(LITERAL.finditer(body))
        if len(INCLUDE.findall(body)) != len(literals):
            raise WorldCheckError(f"Unresolved embedded input in {name}")
        for literal in literals:
            dependency = source.parent / literal[1]
            required.add(regular(root, dependency))
            if literal[0].startswith("include!"):
                pending.append(dependency)
    return tuple(sorted(required))


def verify_compiled(root: Path, entries: Json) -> None:
    match entries:
        case dict() as pinned:
            missing = set(compiled_inputs(root)) - pinned.keys()
            if missing:
                raise WorldCheckError(
                    f"Source closure omits compiled inputs: {missing}"
                )
        case _:
            raise WorldCheckError("Missing compiled source closure")
