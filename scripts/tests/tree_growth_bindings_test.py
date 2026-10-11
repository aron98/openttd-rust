# SPDX-License-Identifier: GPL-2.0-only
"""Current tree integration inputs must be bound without historical relabeling."""

from pathlib import Path

from scripts.context_ci_support import sources
from scripts.grf_control_evidence import text
from scripts.language_ci_compare import mapping
from scripts.owned_restore_sources import verify_compiled
from scripts.tree_growth_inputs import source_names
from scripts.world_check_support import at, read_json

ROOT = Path(__file__).resolve().parents[2]


def test_current_tree_product_in_compiled_layouts() -> None:
    for path in (ROOT / "scripts").rglob("*.json"):
        data = read_json(path)
        if not isinstance(data, dict):
            continue
        for key in ("sources", "source_hashes"):
            entries = data.get(key)
            if (
                not isinstance(entries, dict)
                or "crates/ottd-sim/src/world_ticks.rs" not in entries
            ):
                continue
            verify_compiled(ROOT, entries)
            sources(ROOT, {"sources": entries})


def test_tree_proof_inputs_registered() -> None:
    contract = read_json(ROOT / "compatibility/contract.json")
    assets = at(contract, ("assets",))
    assert isinstance(assets, list)
    registered = {text(mapping(asset)["path"]) for asset in assets}
    required = {
        "crates/ottd-sim/src/world_ticks/trees.rs",
        "crates/ottd-sim/tests/world_trees.rs",
        "crates/ottd-sim/tests/native_world_trees.rs",
        "scripts/tree-growth-mixed-fixture.json",
        "scripts/tests/tree_growth_bindings_test.py",
        "scripts/build-tree-growth-reference.sh",
        "scripts/build-tree-growth-mixed-reference.sh",
    }
    required.update(
        str(path.relative_to(ROOT))
        for path in (ROOT / "scripts").glob("*tree*growth*.py")
    )
    required.update(
        str(path.relative_to(ROOT))
        for path in (ROOT / "reference").glob("tree_growth*")
    )
    assert required <= registered


def test_tree_producer_inventory_matches_current_inputs() -> None:
    inventory = read_json(ROOT / "scripts/tree-growth-source.json")
    entries = mapping(at(inventory, ("sources",)))
    assert set(entries) == set(source_names(ROOT))
    sources(ROOT, inventory)
