"""Check actual branch/lifecycle causality; never normalize divergent outcomes."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import assert_never

from scripts.async_trace_host import Host
from scripts.async_trace_jobs import Jobs
from scripts.async_trace_model import Event, Kind
from scripts.movement_ci.value import EvidenceError, require


@dataclass(slots=True)
class Checker:
    jobs: Jobs = field(default_factory=Jobs)
    host: Host = field(default_factory=Host)
    sample: Event | None = None
    counts: dict[str, int] = field(default_factory=dict)

    def predicate(self, event: Event) -> None:
        self.sample = event
        due, completed = event.value("due"), event.value("completed")
        require("short circuit violated", condition=due is True or completed is None)
        if event.value("job") is not None:
            require(
                "sampled nonfront",
                condition=event.queue("running")[0] == event.number("job"),
            )
            self.jobs.accept(event)

    def request(self, event: Event) -> None:
        require("request lacks branch sample", condition=self.sample is not None)
        if self.sample is not None:
            require(
                "request uses stale sample", condition=self.sample.host == event.host
            )
            pending = (
                self.sample.value("due") is True
                and self.sample.value("completed") is False
            )
            require(
                "request contradicts sample", condition=event.value("pause") is pending
            )

    def pause_control(self, event: Event) -> None:
        match event.kind:
            case Kind.SAMPLE:
                self.predicate(event)
            case Kind.REQUEST:
                self.request(event)
            case Kind.APPLY:
                mask = 1 << event.number("mode")
                previous = event.number("previous")
                expected = (
                    previous | mask
                    if event.value("pause") is True
                    else previous & ~mask
                )
                if expected == 128 and event.value("pause") is False:
                    expected = 0
                require("pause mask transition", condition=event.pause == expected)
            case invalid:
                raise EvidenceError(f"non-pause event {invalid}")

    def accept(self, event: Event) -> None:
        self.counts[event.kind] = self.counts.get(event.kind, 0) + 1
        match event.kind:
            case (
                Kind.ENTER
                | Kind.AFTER
                | Kind.EXIT
                | Kind.LIMIT_ENTER
                | Kind.LIMIT_LEAVE
                | Kind.GAME_ENTER
                | Kind.GAME_LEAVE
            ):
                self.host.accept(event)
                return
            case Kind.SAMPLE | Kind.REQUEST | Kind.APPLY:
                self.pause_control(event)
                return
            case Kind.SPAWN:
                self.jobs.spawn(event)
                return
            case (
                Kind.BACKEND
                | Kind.APPEND
                | Kind.JOIN
                | Kind.DESTRUCTOR
                | Kind.JOINED
                | Kind.PUBLICATION
                | Kind.PUBLISHED
                | Kind.MISSING
                | Kind.FREED
                | Kind.ABORT
                | Kind.ABORTED
            ):
                self.jobs.accept(event)
                return
            case Kind.CLEAR:
                require(
                    "clear retains queues",
                    condition=not event.queue("running")
                    and not event.queue("scheduled"),
                )
                return
            case Kind.REQUEUED | Kind.LOAD_PAUSE:
                return
        assert_never(event.kind)


def check(events: tuple[Event, ...]) -> Checker:
    checker = Checker()
    for sequence, event in enumerate(events):
        require("sequence gap", condition=event.sequence == sequence)
        checker.accept(event)
    require("incomplete host call", condition=checker.host.current is None)
    return checker
