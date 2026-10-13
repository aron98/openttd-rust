"""Typed borrowed observations from the finite native root oracle."""

from dataclasses import dataclass

from scripts.world_check_support import WorldCheckError


class RootTraceError(WorldCheckError):
    """A native root trace violates its complete ordered protocol."""


@dataclass(frozen=True, slots=True)
class ObjectState:
    name: str
    kind: int
    label: str
    references: int
    extent: int
    string: bytes | None


@dataclass(frozen=True, slots=True)
class VmState:
    label: str
    references: int
    primary: bool
    allocated: int
    top: int
    base: int
    varargs: int
    frames: int
    traps: int
    debt: int


@dataclass(frozen=True, slots=True)
class Snapshot:
    phase: str
    vm: VmState
    persistent: tuple[ObjectState, ...]
    stack: tuple[ObjectState, ...]
    varargs: tuple[ObjectState, ...]
    closures: tuple[ObjectState, ...]
    frame_states: tuple[tuple[int, ...], ...]
    chain: str


@dataclass(frozen=True, slots=True)
class RootTrace:
    snapshots: tuple[Snapshot, ...]
    collected: tuple[tuple[str, int], ...]
    objects: tuple[ObjectState, ...]
    free_events: tuple[tuple[str, int], ...]
    lines: tuple[str, ...]


class Cursor:
    """Advance a bounded trace without inventing missing or duplicate records."""

    def __init__(self, raw: bytes) -> None:
        try:
            text = raw.decode("ascii")
        except UnicodeDecodeError as error:
            raise RootTraceError("Root trace is not ASCII") from error
        if not text.endswith("\n") or len(raw) > 2**20:
            raise RootTraceError("Root trace framing or size differs")
        self.lines: tuple[str, ...] = tuple(text.splitlines())
        self.index: int = 0

    def take(self) -> str:
        if self.index >= len(self.lines):
            raise RootTraceError("Truncated root trace")
        value = self.lines[self.index]
        self.index += 1
        return value

    def expect(self, expected: str) -> None:
        if self.take() != expected:
            raise RootTraceError(f"Expected root record: {expected}")


def integer(word: str, low: int = 0, high: int = 2**63 - 1) -> int:
    """Read one canonical bounded native integer."""
    try:
        value = int(word)
    except ValueError as error:
        raise RootTraceError("Invalid root integer") from error
    if str(value) != word or not low <= value <= high:
        raise RootTraceError("Root integer range or spelling differs")
    return value


def label(word: str, *, absent: bool = True) -> str:
    """Accept one stable recorder label or explicit unknown/absent marker."""
    if len(word) != 1 or not (
        "A" <= word <= "Z" or word == "?" or (absent and word == "-")
    ):
        raise RootTraceError("Invalid root identity label")
    return word
