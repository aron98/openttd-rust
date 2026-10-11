# SPDX-License-Identifier: GPL-2.0-only
"""Current Rust tree proof targets, embedded fixtures and Python producer inputs."""

from __future__ import annotations

import ast
from pathlib import Path

from scripts.owned_restore_sources import compiled_inputs, regular


def source_names(root: Path) -> tuple[str, ...]:
    names = set(compiled_inputs(root))
    names.update(
        regular(root, root / name)
        for name in (
            "rust-toolchain.toml",
            "upstream.toml",
            "scripts/reference.cfg",
            "scripts/build-tree-growth-reference.sh",
            "scripts/build-tree-growth-mixed-reference.sh",
            "scripts/tree-growth-mixed-fixture.json",
            "crates/ottd-sim/tests/world_trees.rs",
            "crates/ottd-sim/tests/native_world_trees.rs",
            "crates/ottd-sim/tests/road_movement_ci.rs",
            "crates/ottd-sim/tests/road_movement_ci/controls.rs",
            "fixtures/replay/clear-v362.sav",
        )
    )
    names.update(
        regular(root, path) for path in (root / "reference").glob("tree_growth*")
    )
    pending = [
        "scripts/check-tree-growth.py",
        "scripts/check-tree-growth-mixed.py",
        "scripts/tree_growth_inputs.py",
        "scripts/tests/tree_growth_bindings_test.py",
    ]
    while pending:
        name = pending.pop()
        if name in names:
            continue
        names.add(regular(root, root / name))
        pending.extend(
            node.module.replace(".", "/") + ".py"
            for node in ast.walk(ast.parse((root / name).read_text()))
            if isinstance(node, ast.ImportFrom)
            and node.module
            and node.module.startswith("scripts.")
        )
    return tuple(sorted(names))
