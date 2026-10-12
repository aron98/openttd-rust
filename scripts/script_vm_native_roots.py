"""Parse complete native root snapshots without a projected runtime comparison."""

from typing import Final

from scripts.script_vm_native_roots_model import (
    Cursor,
    ObjectState,
    RootTrace,
    RootTraceError,
    Snapshot,
    VmState,
    integer,
    label,
)

STRING: Final = 0x08000010
TABLE: Final = 0x0A000020
ARRAY: Final = 0x08000040
KINDS: Final = frozenset(
    (
        0x01000001,
        0x05000002,
        0x05000004,
        0x01000008,
        STRING,
        TABLE,
        ARRAY,
        0x0A000080,
        0x08000100,
        0x08000200,
        0x08000400,
        0x800,
        0x08001000,
        0x08002000,
        0x08004000,
        0x0A008000,
        0x08010000,
    )
)


def object_state(cursor: Cursor, line: str, expected: str | None = None) -> ObjectState:
    """Read a type-dispatched borrowed value and its exact optional string bytes."""
    words = line.split(" ")
    if len(words) != 7 or words[:2] != ["roots", "object"]:
        raise RootTraceError("Malformed root object")
    name, kind, identity, refs, extent = words[2:]
    if expected is not None and name != expected:
        raise RootTraceError("Root field order differs")
    native_kind = integer(kind, high=2**31 - 1)
    if native_kind not in KINDS or not name or not name.replace("-", "").isalnum():
        raise RootTraceError("Unknown root field or type")
    references = integer(refs, high=2**64 - 1)
    size = integer(extent, low=-1)
    if (native_kind in (TABLE, ARRAY)) != (size >= 0):
        raise RootTraceError("Object extent does not match native type")
    if not native_kind & 0x08000000 and (references != 0 or identity != "-"):
        raise RootTraceError("Scalar has a reference owner")
    data: bytes | None = None
    if native_kind == STRING:
        fields = cursor.take().split(" ")
        if len(fields) != 5 or fields[:3] != ["roots", "bytes", name]:
            raise RootTraceError("Missing native string bytes")
        size_bytes = integer(fields[3])
        try:
            data = bytes.fromhex(fields[4])
        except ValueError as error:
            raise RootTraceError("Invalid native string hex") from error
        if data.hex() != fields[4] or len(data) != size_bytes:
            raise RootTraceError("Native string length differs")
    return ObjectState(name, native_kind, label(identity), references, size, data)


def vm_state(cursor: Cursor) -> VmState:
    """Read physical VM counters separately from child enumeration."""
    words = cursor.take().split(" ")
    if len(words) != 12 or words[:2] != ["roots", "vm"]:
        raise RootTraceError("Missing VM root record")
    numbers = tuple(integer(x, low=-(2**63)) for x in words[3:])
    refs, primary, allocated, top, base, varargs, frames, traps, debt = numbers
    if (
        primary not in (0, 1)
        or min(refs, allocated, top, base, varargs, frames, traps) < 0
    ):
        raise RootTraceError("Invalid VM root counters")
    if max(allocated, varargs, frames) > 4096 or not base <= top <= allocated:
        raise RootTraceError("Invalid physical VM span")
    return VmState(
        label(words[2]),
        refs,
        bool(primary),
        allocated,
        top,
        base,
        varargs,
        frames,
        traps,
        debt,
    )


def snapshot(cursor: Cursor, phase: str) -> Snapshot:
    """Require the exact VM root order and every allocated physical slot."""
    vm = vm_state(cursor)
    allocated, varargs, frames = vm.allocated, vm.varargs, vm.frames
    persistent = tuple(
        object_state(cursor, cursor.take(), name)
        for name in ("error", "handler", "debug", "root", "temporary")
    )
    slots: list[tuple[ObjectState, ...]] = []
    for name, count in (("stack", allocated), ("vararg", varargs)):
        values: list[ObjectState] = []
        for index in range(count):
            cursor.expect(f"roots {name} {index}")
            values.append(object_state(cursor, cursor.take(), "slot"))
        slots.append(tuple(values))
    closures: list[ObjectState] = []
    frame_states: list[tuple[int, ...]] = []
    for index in range(frames):
        fields = cursor.take().split(" ")
        if len(fields) != 7 or fields[:3] != ["roots", "frame", str(index)]:
            raise RootTraceError("Active call-frame order differs")
        state = tuple(integer(x, high=2_147_483_647) for x in fields[3:])
        if state[0] not in (0, 1) or max(state[2:]) > 65535:
            raise RootTraceError("Call-frame native widths differ")
        frame_states.append(state)
        closures.append(object_state(cursor, cursor.take(), "closure"))
    fields = cursor.take().split(" ")
    if len(fields) != 4 or fields[:2] != ["roots", "chain"]:
        raise RootTraceError("Missing root chain")
    count, chain = integer(fields[2]), fields[3]
    if count != len(chain):
        raise RootTraceError("Root chain count differs")
    for identity in chain:
        _ = label(identity, absent=False)
    known = chain.replace("?", "")
    if len(known) != len(set(known)):
        raise RootTraceError("Duplicate chain identity")
    cursor.expect(f"roots end {phase}")
    return Snapshot(
        phase,
        vm,
        persistent,
        slots[0],
        slots[1],
        tuple(closures),
        tuple(frame_states),
        chain,
    )


def parse(raw: bytes, *, active: bool = False) -> RootTrace:
    """Require complete snapshots and a successful unchanged free-recorder ending."""
    cursor = Cursor(raw)
    snapshots: list[Snapshot] = []
    collected: list[tuple[str, int]] = []
    objects: list[ObjectState] = []
    frees: list[tuple[str, int]] = []
    while cursor.index < len(cursor.lines) - 1:
        line = cursor.take()
        fields = line.split(" ")
        match fields:
            case ["roots", "begin", phase]:
                snapshots.append(snapshot(cursor, phase))
            case ["roots", "collected", phase, count]:
                collected.append((phase, integer(count)))
            case ["roots", "object", *_]:
                objects.append(object_state(cursor, line))
            case ["free", identity, size]:
                if identity == "?" or any(old == identity for old, _ in frees):
                    raise RootTraceError("Unknown or repeated retirement identity")
                frees.append((label(identity, absent=False), integer(size, low=1)))
            case (
                ["roots", "result", "1", "integer", "1"]
                | ["roots", "compile", "failure", "1"]
                | ["roots", "release", "intermediate", "0"]
                | ["roots", "release", "last", "1"]
                | ["roots", "detached-head", "1", "-"]
            ):
                continue
            case _:
                if not active or line.startswith("roots ") or not line:
                    raise RootTraceError(f"Unexpected root trace line: {line}")
    cursor.expect("recorder 1")
    if not snapshots or len({x.phase for x in snapshots}) != len(snapshots):
        raise RootTraceError("Missing or duplicate snapshot phase")
    return RootTrace(
        tuple(snapshots), tuple(collected), tuple(objects), tuple(frees), cursor.lines
    )
