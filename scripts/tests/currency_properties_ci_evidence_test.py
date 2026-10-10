from __future__ import annotations

from copy import deepcopy
from pathlib import Path

import pytest

from scripts.currency_properties_ci_evidence import byte_owners, validate
from scripts.world_check_support import Json, WorldCheckError


def owners() -> list[Json]:
    return [{"prefix": [237, 160, 128], "suffix": []} for _ in range(46)]


def test_byte_owner_projection_accepts_defined_surrogate_bytes() -> None:
    byte_owners(owners())


@pytest.mark.parametrize("symbol", ["replacement", [256], [-1], [True], None])
def test_byte_owner_projection_rejects_nonbyte_transport(symbol: Json) -> None:
    rows = owners()
    rows[0] = {"prefix": deepcopy(symbol), "suffix": []}
    with pytest.raises(WorldCheckError):
        byte_owners(rows)


def test_byte_owner_projection_requires_complete_owner_table() -> None:
    with pytest.raises(WorldCheckError):
        byte_owners(owners()[:-1])


def test_unverified_layout_refuses_before_artifact_access(tmp_path: Path) -> None:
    with pytest.raises(WorldCheckError, match="awaits actual"):
        validate(
            tmp_path,
            tmp_path / "absent",
            tmp_path / "oracle",
            {"verified_native_corpus": False},
        )
