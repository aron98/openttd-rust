"""Decode passive observer events without conflating host, game or network clocks."""

from __future__ import annotations

from dataclasses import dataclass
from enum import StrEnum
from pathlib import Path

from scripts.movement_ci.value import DECODE, Json, array, field, integer, string


class Kind(StrEnum):
    ENTER = "host_enter"
    EXIT = "host_exit"
    AFTER = "after_pause_control"
    LIMIT_ENTER = "paused_landscaping_limits_enter"
    LIMIT_LEAVE = "paused_landscaping_limits_leave"
    GAME_ENTER = "paused_game_loop_enter"
    GAME_LEAVE = "paused_game_loop_leave"
    SAMPLE = "join_predicate"
    REQUEST = "pause_request"
    APPLY = "pause_apply"
    SPAWN = "spawn_begin"
    BACKEND = "backend"
    APPEND = "running_append"
    JOIN = "join_begin"
    DESTRUCTOR = "destructor_join_begin"
    JOINED = "joined"
    PUBLICATION = "publication_begin"
    PUBLISHED = "publication_end"
    MISSING = "publication_graph_missing"
    FREED = "freed"
    REQUEUED = "requeued"
    ABORT = "abort_begin"
    ABORTED = "abort_applied"
    CLEAR = "schedule_cleared"
    LOAD_PAUSE = "afterload_pause_set"


@dataclass(frozen=True, slots=True)
class Event:
    kind: Kind
    sequence: int
    host: int
    tick: int
    pause: int
    data: Json

    def value(self, key: str) -> Json:
        return field(self.data, key)

    def number(self, key: str) -> int:
        return integer(self.value(key))

    def queue(self, key: str) -> tuple[int, ...]:
        return tuple(integer(item) for item in array(field(self.value("queues"), key)))

    def has(self, key: str) -> bool:
        match self.data:
            case dict() as data:
                return key in data
            case _:
                return False


def read(path: Path) -> tuple[Event, ...]:
    events: list[Event] = []
    with path.open() as source:
        for line in source:
            data = DECODE(line)
            runtime = field(data, "runtime")
            events.append(
                Event(
                    Kind(string(field(data, "event"))),
                    integer(field(data, "sequence")),
                    integer(field(data, "host")),
                    integer(field(runtime, "tick")),
                    integer(field(runtime, "pause")),
                    data,
                ),
            )
    return tuple(events)
