import pytest

from scripts.owned_restore_capacity import array, full_lists, gamma
from scripts.world_check_support import WorldCheckError


def test_capacity_preserves_schema_and_other_chunks() -> None:
    header = b"OTTN\x01j\0\0"
    prefix = b"ABCD\0\0\0\x03xyz"
    source = header + prefix + b"ORDL\x03\x02S\x01\x03AB\0" + bytes(4)
    prepared = full_lists(source)
    assert prepared.startswith(header + prefix + b"ORDL\x03\x02S")
    frames, end = array(prepared, len(header + prefix + b"ORDL\x03"))
    assert len(frames) == 64001
    assert frames[:3] == [b"\x02S", b"\x02\0", b"\x03AB"]
    assert all(row == b"\x02\0" for row in frames[3:])
    assert prepared[end:] == bytes(4)


@pytest.mark.parametrize("data", [b"", b"\x80", b"\xf8", b"\xe0\0"])
def test_gamma_truncation(data: bytes) -> None:
    with pytest.raises(WorldCheckError):
        _ = gamma(data, 0)


@pytest.mark.parametrize("mode", [2, 4])
def test_sparse_list_rejected(mode: int) -> None:
    source = b"OTTN\x01j\0\0ORDL" + bytes([mode]) + b"\x02S\0" + bytes(4)
    with pytest.raises(WorldCheckError, match="ordinary ORDL"):
        _ = full_lists(source)


def test_absent_list_rejected() -> None:
    with pytest.raises(WorldCheckError, match="ORDL membership"):
        _ = full_lists(b"OTTN\x01j\0\0" + bytes(4))
