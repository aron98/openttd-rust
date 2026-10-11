"""Validate job lifetimes and safely authorized completed-result snapshots."""

from __future__ import annotations

from dataclasses import dataclass, field

from scripts.async_trace_model import Event, Kind
from scripts.movement_ci.value import EvidenceError, Json, require, same


@dataclass(slots=True)
class Job:
    identity: int
    aborted: bool = False
    joined: bool = False
    discarded: bool = False
    publishing: bool = False
    published: bool = False
    annotation: Json = None

    def annotations(self, event: Event) -> None:
        if not event.has("annotations"):
            return
        require("aborted annotations", condition=not self.aborted)
        require(
            "unauthorized annotations",
            condition=(event.kind is Kind.SAMPLE and event.value("completed") is True)
            or (event.kind is Kind.JOINED and event.value("cleaning") is False),
        )
        if self.annotation is not None:
            require(
                "completed annotations changed",
                condition=same(self.annotation, event.value("annotations")),
            )
        self.annotation = event.value("annotations")

    def transition(self, event: Event) -> None:
        match event.kind:
            case Kind.ABORT:
                self.aborted = True
            case Kind.ABORTED:
                require("abort ordering", condition=self.aborted)
            case Kind.JOINED:
                self.joined = True
                self.discarded = event.value("cleaning") is True
            case Kind.PUBLICATION:
                require(
                    "publication before join/after abort",
                    condition=self.joined and not self.aborted,
                )
                self.publishing = True
            case Kind.PUBLISHED:
                require("publication end without begin", condition=self.publishing)
                self.published = True
            case Kind.MISSING:
                require("missing graph before join", condition=self.joined)
                self.published = True
            case Kind.APPEND:
                require(
                    "running append missing",
                    condition=event.number("job") in event.queue("running"),
                )
            case Kind.BACKEND | Kind.JOIN | Kind.DESTRUCTOR | Kind.SAMPLE | Kind.FREED:
                pass
            case invalid:
                raise EvidenceError(f"non-lifecycle event {invalid}")


@dataclass(slots=True)
class Jobs:
    live: dict[int, Job] = field(default_factory=dict)

    def spawn(self, event: Event) -> None:
        identity = event.number("job")
        previous = self.live.get(identity)
        require(
            "spawn reuses live job", condition=previous is None or previous.discarded
        )
        if previous is not None:
            require(
                "reused observer lifetime",
                condition=previous.identity != event.number("lifetime"),
            )
        self.live[identity] = Job(event.number("lifetime"))
        require("missing owned input", condition=event.has("input"))

    def accept(self, event: Event) -> None:
        identity = event.number("job")
        job = self.live[identity]
        require("wrong lifetime", condition=job.identity == event.number("lifetime"))
        job.annotations(event)
        job.transition(event)
        if event.kind is Kind.FREED:
            require("free before publication", condition=job.published)
            require(
                "freed job running", condition=identity not in event.queue("running")
            )
            del self.live[identity]
