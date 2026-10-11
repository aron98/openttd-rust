"""Missing or tampered observer helpers must never receive route proof credit."""

from pathlib import Path

import pytest

from scripts.cargo_observer_inputs import REQUIRED, verify
from scripts.gameplay_foundations import digest
from scripts.world_check_support import Json, WorldCheckError


@pytest.mark.parametrize("name", REQUIRED)
def test_missing_helper_is_rejected(name: str) -> None:
    root = Path(__file__).resolve().parents[2]
    hashes: dict[str, Json] = {
        source: digest(root / source) for source in REQUIRED if source != name
    }
    with pytest.raises(WorldCheckError, match="omits required helpers"):
        verify(root, {"sources": hashes})


@pytest.mark.parametrize("name", REQUIRED)
def test_tampered_helper_bytes_are_rejected(name: str, tmp_path: Path) -> None:
    root = Path(__file__).resolve().parents[2]
    hashes: dict[str, Json] = {source: digest(root / source) for source in REQUIRED}
    for source in REQUIRED:
        destination = tmp_path / source
        destination.parent.mkdir(parents=True, exist_ok=True)
        _ = destination.write_bytes((root / source).read_bytes())
    _ = (tmp_path / name).write_bytes((root / name).read_bytes() + b"\n")
    with pytest.raises(WorldCheckError, match="Pinned witness source changed"):
        verify(tmp_path, {"sources": hashes})


def test_complete_current_helpers_are_accepted() -> None:
    root = Path(__file__).resolve().parents[2]
    hashes: dict[str, Json] = {source: digest(root / source) for source in REQUIRED}
    verify(root, {"sources": hashes})
