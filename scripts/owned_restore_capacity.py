from __future__ import annotations

from scripts.gameplay_foundations import digest
from scripts.owned_restore_run import RestoreRun
from scripts.world_check_support import WorldCheckError, at, read_json, run, write_json


def gamma(data: bytes, offset: int) -> tuple[int, int]:
    if offset >= len(data):
        raise WorldCheckError("Truncated capacity fixture gamma")
    first = data[offset]
    offset += 1
    width = 0
    mask = 0x7F
    while first & (0x80 >> width):
        width += 1
        mask >>= 1
        if width == 5:
            raise WorldCheckError("Invalid capacity fixture gamma")
    if offset + width > len(data):
        raise WorldCheckError("Truncated capacity fixture gamma body")
    value = first & mask
    for byte in data[offset : offset + width]:
        value = (value << 8) | byte
    return value, offset + width


def array(data: bytes, offset: int) -> tuple[list[bytes], int]:
    frames: list[bytes] = []
    while True:
        start = offset
        length, offset = gamma(data, offset)
        if length == 0:
            return frames, offset
        offset += length - 1
        if offset > len(data):
            raise WorldCheckError("Truncated capacity fixture table")
        frames.append(data[start:offset])


def full_lists(data: bytes) -> bytes:
    """Fill ordinary ORDL table vacancies with wire empty-vector records."""
    if data[:8] != b"OTTN\x01j\0\0":
        raise WorldCheckError("Capacity requires uncompressed vanilla version 362")
    offset = 8
    tables: list[tuple[int, int, list[bytes]]] = []
    while offset + 5 <= len(data) and data[offset : offset + 4] != bytes(4):
        identity = data[offset : offset + 4]
        mode = data[offset + 4]
        offset += 5
        if mode & 15 == 0:
            if offset + 3 > len(data):
                raise WorldCheckError("Truncated capacity RIFF header")
            length = int.from_bytes(
                bytes([mode >> 4]) + data[offset : offset + 3], "big"
            )
            offset += 3 + length
            continue
        if mode not in (1, 2, 3, 4):
            raise WorldCheckError("Unsupported capacity fixture chunk mode")
        start = offset
        frames, offset = array(data, offset)
        if identity == b"ORDL":
            if mode != 3:
                raise WorldCheckError("Capacity requires ordinary ORDL table")
            tables.append((start, offset, frames))
    if offset + 4 != len(data) or data[offset:] != bytes(4) or len(tables) != 1:
        raise WorldCheckError("Capacity fixture framing or ORDL membership differs")
    start, end, frames = tables[0]
    if not 1 <= len(frames) <= 64001:
        raise WorldCheckError("Capacity fixture ORDL row count invalid")
    rows = [b"\x02\x00" if row == b"\x01" else row for row in frames[1:]]
    rows.extend([b"\x02\x00"] * (64000 - len(rows)))
    return data[:start] + frames[0] + b"".join(rows) + b"\0" + data[end:]


def prepare_capacity(job: RestoreRun) -> None:
    results = job.output / "results"
    source = results / "matrix/copies.sav"
    destination = results / "preparation/capacity.sav"
    before = digest(source)
    _ = destination.write_bytes(full_lists(source.read_bytes()))
    for name, path in (("original", source), ("prepared", destination)):
        exported = run(
            [job.executable("cli"), "world", str(path), "--view", "saved"],
            results / f"capacity-{name}-export",
        )
        _ = (results / f"capacity-{name}.json").write_text(exported.stdout)
    original = read_json(results / "capacity-original.json")
    actual = read_json(results / "capacity-prepared.json")
    old = at(original, ("chunks", "ORDL", "records"))
    new = at(actual, ("chunks", "ORDL", "records"))
    match old, new:
        case dict() as previous, dict() as current:
            if (
                set(current) != {str(index) for index in range(64000)}
                or any(current[key] != row for key, row in previous.items())
                or any(
                    row != {"orders": []}
                    for key, row in current.items()
                    if key not in previous
                )
            ):
                raise WorldCheckError("Prepared capacity rows differ")
            current.clear()
            current.update(previous)
        case _:
            raise WorldCheckError("Missing capacity rows")
    if actual != original or digest(source) != before:
        raise WorldCheckError("Capacity preparation changed unrelated fields or source")
    write_json(
        results / "capacity-preparation.json",
        {
            "source_sha256": before,
            "prepared_sha256": digest(destination),
            "records": 64000,
            "scope": "saved-input fixture only",
        },
    )
