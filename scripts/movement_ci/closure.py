"""Bind compiled, embedded-test, harness and native build inputs before producers."""

from __future__ import annotations

from pathlib import Path

from scripts.owned_restore_sources import INCLUDE, LITERAL, compiled_inputs, regular

from .baseline import FileInput
from .value import require


def sources(root: Path) -> tuple[FileInput, ...]:
    names = set(compiled_inputs(root))
    pending = sorted((root / "crates").glob("*/tests/**/*.rs"))
    inspected: set[str] = set()
    while pending:
        path = pending.pop()
        name = regular(root, path)
        names.add(name)
        if name in inspected:
            continue
        inspected.add(name)
        body = path.read_text()
        literals = list(LITERAL.finditer(body))
        require(
            "unresolved test embedded input: " + name,
            condition=len(INCLUDE.findall(body)) == len(literals),
        )
        for literal in literals:
            dependency = path.parent / literal[1]
            names.add(regular(root, dependency))
            if literal[0].startswith("include!"):
                pending.append(dependency)
    for path in root.iterdir():
        if path.is_file():
            names.add(regular(root, path))
    for folder in (
        ".github",
        ".cargo",
        "compatibility",
        "crates",
        "docs",
        "fixtures",
        "scripts",
        "reference",
        "cmake",
    ):
        for path in sorted((root / folder).rglob("*")):
            if path.is_file() and "__pycache__" not in path.parts:
                names.add(regular(root, path))
    return tuple(FileInput.capture(root / name) for name in sorted(names))


def verify(inputs: tuple[FileInput, ...]) -> None:
    require("missing complete source closure", condition=bool(inputs))
    require(
        "duplicate source input",
        condition=len({item.path for item in inputs}) == len(inputs),
    )
    for item in inputs:
        item.verify()
