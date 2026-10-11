# SPDX-License-Identifier: GPL-2.0-only
"""Exact current spreading producer inputs, including embedded regression saves."""

from __future__ import annotations

import ast
from pathlib import Path

from scripts.owned_restore_sources import regular
from scripts.tree_growth_inputs import source_names as growth_sources


def source_names(root: Path) -> tuple[str, ...]:
    names = set(growth_sources(root))
    for pattern in [
        "crates/ottd-sim/tests/*tree_spread*.rs",
        "crates/ottd-sim/tests/native_world_tree_spread/*.rs",
        "fixtures/tree-spread/*",
        "reference/tree_spread*",
    ]:
        names.update(regular(root, path) for path in root.glob(pattern))
    names.update(
        [
            regular(root, root / "scripts/build-tree-spread-reference.sh"),
            regular(root, root / "scripts/tree-spread-road-prepare.json"),
            regular(root, root / "scripts/grf-control-ruff.toml"),
        ]
    )
    pending = [
        "scripts/check-tree-spread.py",
        "scripts/tests/tree_spread_bindings_test.py",
        *(
            str(path.relative_to(root))
            for path in (root / "scripts").glob("tree_spread_*.py")
        ),
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
