"""Check host-call boundaries and the exact paused callback sequence."""

from __future__ import annotations

from dataclasses import dataclass, field

from scripts.async_trace_model import Event, Kind
from scripts.movement_ci.value import EvidenceError, require


@dataclass(slots=True)
class Host:
    current: Event | None = None
    after: Event | None = None
    callbacks: list[Kind] = field(default_factory=list)
    paused: int = 0
    last: int = 0

    def accept(self, event: Event) -> None:
        match event.kind:
            case Kind.ENTER:
                require("nested host call", condition=self.current is None)
                require("host ordinal gap", condition=event.host == self.last + 1)
                self.last = event.host
                self.current, self.after = event, None
                self.callbacks.clear()
            case Kind.AFTER:
                require("after pause without entry", condition=self.current is not None)
                self.after = event
            case Kind.EXIT:
                require(
                    "host exit without after pause", condition=self.after is not None
                )
                if self.after is not None and self.after.pause:
                    require(
                        "paused tick advanced", condition=event.tick == self.after.tick
                    )
                    self.paused += 1
                self.current = None
            case (
                Kind.LIMIT_ENTER | Kind.LIMIT_LEAVE | Kind.GAME_ENTER | Kind.GAME_LEAVE
            ):
                require(
                    "callback outside paused branch", condition=self.after is not None
                )
                self.callbacks.append(event.kind)
                expected = [
                    Kind.LIMIT_ENTER,
                    Kind.LIMIT_LEAVE,
                    Kind.GAME_ENTER,
                    Kind.GAME_LEAVE,
                ]
                require(
                    "paused callback order",
                    condition=self.callbacks == expected[: len(self.callbacks)],
                )
            case invalid:
                raise EvidenceError(f"non-host event {invalid}")
