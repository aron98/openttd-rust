# SPDX-License-Identifier: GPL-2.0-only
"""Spreading inputs and current source closure must remain explicitly bound."""

from pathlib import Path

from scripts.context_ci_support import sources
from scripts.grf_control_evidence import text
from scripts.language_ci_compare import mapping
from scripts.tree_spread_inputs import source_names
from scripts.world_check_support import at, read_json

ROOT = Path(__file__).resolve().parents[2]


def test_spreading_producer_exact_source_roster() -> None:
    inventory = read_json(ROOT / "scripts/tree-spread-source.json")
    assert set(mapping(at(inventory, ("sources",)))) == set(source_names(ROOT))
    sources(ROOT, inventory)


def test_spreading_assets_registered_with_no_omissions() -> None:
    assets = at(read_json(ROOT / "compatibility/contract.json"), ("assets",))
    assert isinstance(assets, list)
    registered = {text(mapping(asset)["path"]) for asset in assets}
    required = {
        "scripts/tree-spread-source.json",
        "scripts/tree-spread-road-prepare.json",
        "scripts/build-tree-spread-reference.sh",
        "scripts/check-tree-spread.py",
        "scripts/tests/tree_spread_bindings_test.py",
        "crates/ottd-sim/tests/native_world_tree_spread.rs",
        "crates/ottd-sim/tests/native_world_tree_spread/trace.rs",
        "crates/ottd-sim/tests/world_tree_spread.rs",
        "crates/ottd-sim/src/world_ticks/trees/placement.rs",
        "crates/ottd-sim/src/world_ticks/trees/counter_tests.rs",
    }
    for pattern in [
        "scripts/tree_spread_*.py",
        "reference/tree_spread*",
        "fixtures/tree-spread/*",
    ]:
        required.update(str(path.relative_to(ROOT)) for path in ROOT.glob(pattern))
    assert required <= registered
