"""Source-bound scenario and passive-recorder checks for native root traces."""

from typing import Final

from scripts.script_vm_native_roots import TABLE, parse
from scripts.script_vm_native_roots_model import RootTrace, RootTraceError

PHASES: Final = {
    "shared": ("idle", "shared-collected"),
    "idle": ("idle", "idle-collected"),
    "persistent": ("retained", "compile-error", "compile-success", "reset"),
    "replace": ("before", "replaced"),
    "same": ("before", "replaced"),
    "friend-stack": (
        "friend-before",
        "primary-collected",
        "friend-after",
        "friend-released",
    ),
    "friend-local": ("friend-before", "primary-collected", "friend-after"),
    "friend-host": (
        "friend-before",
        "primary-collected",
        "friend-after",
        "friend-released",
    ),
    "refs": ("tokens", "retained-tokens", "reused-grown", "tokens-released"),
    "refs-string": ("tokens", "retained-tokens", "reused-grown", "tokens-released"),
}


def scenario(mode: str, raw: bytes) -> RootTrace:
    """Require reached source roles without inventing a private RefTable census."""
    trace = parse(raw)
    if tuple(x.phase for x in trace.snapshots) != PHASES.get(mode):
        raise RootTraceError("Scenario snapshot roster differs")
    context_roles(trace)
    if mode in {"shared", "idle"}:
        for item in trace.snapshots:
            root = item.persistent[3]
            if root.kind != TABLE or root.references != 1:
                raise RootTraceError("Idle root acquired an extra owner")
    if mode == "shared":
        shared_roots(trace)
    if mode == "friend-local":
        after = trace.snapshots[2]
        if any(value.kind != 0x01000001 for value in after.persistent):
            raise RootTraceError("Finalized idle friend retained a persistent root")
        if "roots detached-head 1 -" not in trace.lines:
            raise RootTraceError("Missing bounded friend retirement head witness")
    return trace


def context_roles(trace: RootTrace) -> None:
    """Validate physical idle contexts and their primary/friend role."""
    for item in trace.snapshots:
        friend = item.phase in {"friend-before", "friend-after"}
        if item.vm.label != ("F" if friend else "V") or item.vm.primary == friend:
            raise RootTraceError("Primary/friend identity differs")
        if item.vm.references != 1 or item.vm.allocated != 256:
            raise RootTraceError("Native idle context ownership or allocation differs")


def shared_roots(trace: RootTrace) -> None:
    """Check the actual thirteen table fields, each with its sole shared owner."""
    if tuple(x.name for x in trace.objects) != tuple("RCMTASNFGHKIW"):
        raise RootTraceError("Shared root declaration/order differs")
    if any(
        x.kind != TABLE or x.references != 1 or x.name != x.label for x in trace.objects
    ):
        raise RootTraceError("Shared root ownership/identity differs")


def recorder_pair(mode: str, off: bytes, on: bytes) -> RootTrace:
    """Require endpoint neutrality while retaining all actual retirement events."""
    left, right = scenario(mode, off), scenario(mode, on)
    if left.free_events or not right.free_events:
        raise RootTraceError("Recorder off/on retirement evidence differs")
    if left.lines != tuple(
        line for line in right.lines if not line.startswith("free ")
    ):
        raise RootTraceError("Passive recorder changed a native endpoint")
    return right


def active_pair(off: bytes, on: bytes, old: bytes) -> RootTrace:
    """Validate added snapshots against the unchanged legacy active action."""
    left, right = parse(off, active=True), parse(on, active=True)
    if tuple(x.phase for x in right.snapshots) != ("before", "after"):
        raise RootTraceError("Active snapshot roster differs")
    if left.free_events or not right.free_events:
        raise RootTraceError("Active recorder evidence missing")
    if left.lines != tuple(
        line for line in right.lines if not line.startswith("free ")
    ):
        raise RootTraceError("Active recorder changed endpoints")
    legacy = b"".join(
        line + b"\n" for line in on.splitlines() if not line.startswith(b"roots ")
    )
    if legacy != old:
        raise RootTraceError("Root snapshot changed the legacy active protocol")
    return right
