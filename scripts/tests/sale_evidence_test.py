# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = ["pytest"]
# ///
# Run: uv run --with pytest python -m pytest scripts/tests/sale_evidence_test.py
"""Sale evidence admission failures."""

from pathlib import Path

import pytest

from scripts.sale_evidence import validate_summary
from scripts.world_check_support import WorldCheckError, write_json


def test_subset_is_rejected(tmp_path: Path) -> None:
    # Given a success marker from only one scenario.
    write_json(tmp_path / "summary.json", {"passed": True, "cases": ["reuse"]})
    # When admitting it as complete sale evidence, then reject it.
    with pytest.raises(WorldCheckError):
        validate_summary(tmp_path)


@pytest.mark.parametrize(
    "output",
    [
        "test result: ok. 0 passed; 0 failed; 0 ignored;",
        "test other ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored;",
    ],
)
def test_empty_or_wrong_test_cannot_be_admitted(output: str) -> None:
    from scripts.gameplay_foundations import require_test

    with pytest.raises(WorldCheckError):
        require_test(output, "cleanup::prepare_sale_cleanup")


def test_wrong_binary_hash_is_rejected(tmp_path: Path) -> None:
    from scripts.sale_provenance import binding

    binary = tmp_path / "native_sale"
    _ = binary.write_bytes(b"changed executable")
    with pytest.raises(WorldCheckError):
        _ = binding(
            f"Running tests/native_sale.rs ({binary})\n",
            {"path": str(binary), "sha256": "0" * 64},
            tmp_path,
        )


def test_equal_count_wrong_membership_is_rejected(tmp_path: Path) -> None:
    from scripts.purchase_evidence import validate_paths

    _ = (tmp_path / "wrong.json").write_text("{}")
    with pytest.raises(WorldCheckError):
        _ = validate_paths(tmp_path, {"required.json"})
