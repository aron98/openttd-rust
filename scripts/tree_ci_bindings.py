from __future__ import annotations

import ast
import shutil
from pathlib import Path

from scripts.context_ci_support import sources
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import text
from scripts.language_ci_compare import compare, mapping
from scripts.owned_restore_sources import compiled_inputs, verify_compiled
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


def source_names(root: Path) -> list[str]:
    names = set(compiled_inputs(root))
    names.update(
        mapping(
            at(
                read_json(root / "scripts/tree-terrain/recipes.json"),
                ("tracked_inputs",),
            )
        ).keys()
    )
    names.update(
        (
            "upstream.toml",
            "rust-toolchain.toml",
            "scripts/setup-snapshot-reference.sh",
            "scripts/reference.cfg",
            "scripts/check-replay-native.cmake",
            "scripts/check-replay-build.cmake",
            "scripts/check-replay-evidence.cmake",
        )
    )
    names.update(str(p.relative_to(root)) for p in (root / "reference").glob("*.hpp"))
    names.update(str(p.relative_to(root)) for p in (root / "reference").glob("*.patch"))
    names.update(
        str(p.relative_to(root))
        for p in (root / "scripts/tree-terrain").rglob("*")
        if p.is_file()
    )
    pending = ["scripts/check-tree-terrain-ci.py"]
    while pending:
        name = pending.pop()
        if name in names:
            continue
        names.add(name)
        pending.extend(
            node.module.replace(".", "/") + ".py"
            for node in ast.walk(ast.parse((root / name).read_text()))
            if isinstance(node, ast.ImportFrom)
            and node.module
            and node.module.startswith("scripts.")
        )
    names.discard("scripts/tree-terrain/layout.json")
    return sorted(names)


def capture(root: Path, output: Path, oracle: Path) -> Json:
    names = source_names(root)
    hashes: dict[str, Json] = {}
    for name in names:
        path = root / name
        if path.is_symlink() or not path.resolve().is_relative_to(root.resolve()):
            raise WorldCheckError("Escaped tree CI source")
        hashes[name] = digest(path)
        target = output / "source" / name
        target.parent.mkdir(parents=True, exist_ok=True)
        _ = shutil.copy2(path, target)
    verify_compiled(root, hashes)
    result: Json = {
        "source_hashes": hashes,
        "native_binary": str(oracle),
        "native_sha256": digest(oracle),
        "native_stamp_sha256": digest(oracle.parent / "replay-build.sha256"),
        "layout_sha256": digest(root / "scripts/tree-terrain/layout.json"),
    }
    write_json(output / "provenance.json", result)
    return result


def verify(root: Path, output: Path, oracle: Path) -> None:
    provenance = read_json(output / "provenance.json")
    hashes = at(provenance, ("source_hashes",))
    compare([*sorted(mapping(hashes))], [*source_names(root)])
    verify_compiled(root, hashes)
    sources(root, {"sources": hashes})
    compare(digest(oracle), at(provenance, ("native_sha256",)))
    compare(
        digest(oracle.parent / "replay-build.sha256"),
        at(provenance, ("native_stamp_sha256",)),
    )
    for table in (
        read_json(output / "test-binaries.json"),
        {"ottd": read_json(output / "cli-binary.json")},
    ):
        for row in mapping(table).values():
            sha = at(row, ("sha256",))
            compare(digest(Path(text(at(row, ("original",))))), sha)
            compare(digest(Path(text(at(row, ("retained",))))), sha)
