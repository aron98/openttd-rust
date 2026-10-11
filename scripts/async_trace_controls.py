"""Reject semantic tampering in genuine captured traces, then recheck originals."""

from __future__ import annotations

import json
import sys
from dataclasses import dataclass
from pathlib import Path

from scripts.async_trace_check import check
from scripts.async_trace_model import Event, Kind, read
from scripts.movement_ci.value import DECODE, EvidenceError, Json, require
from scripts.world_check_support import replace


@dataclass(frozen=True, slots=True)
class Mutation:
    name: str
    source: str
    kind: Kind
    path: tuple[str | int, ...]
    value: Json
    diagnostic: str


def apply(control: Mutation, root: Path, output: Path) -> str:
    source = root / control.source / "async.jsonl"
    events = read(source)
    _ = check(events)
    eligible = [event for event in events if event.kind is control.kind]
    if control.name == "completion":
        eligible = [event for event in eligible if event.value("completed") is True]
    require("missing genuine mutation target", condition=bool(eligible))
    target: Event = eligible[0]
    lines = source.read_text().splitlines()
    payload = DECODE(lines[target.sequence])
    replace(payload, control.path, control.value)
    lines[target.sequence] = json.dumps(payload)
    mutated = output / f"{control.name}.jsonl"
    _ = mutated.write_text("\n".join(lines) + "\n")
    try:
        _ = check(read(mutated))
    except EvidenceError as error:
        require("wrong rejection", condition=control.diagnostic in str(error))
        _ = check(read(source))
        return str(error)
    raise EvidenceError(f"semantic mutation accepted: {control.name}")


def main() -> None:
    require("evidence output arguments", condition=len(sys.argv) == 3)
    root, output = (Path(arg).resolve() for arg in sys.argv[1:])
    output.mkdir(parents=True, exist_ok=False)
    controls = (
        Mutation(
            "completion",
            "wood-on",
            Kind.SAMPLE,
            ("completed",),
            value=False,
            diagnostic="unauthorized annotations",
        ),
        Mutation(
            "pause-mask",
            "paused-on",
            Kind.APPLY,
            ("runtime", "pause"),
            64,
            "pause mask transition",
        ),
        Mutation(
            "running-publication",
            "wood-on",
            Kind.APPEND,
            ("queues", "running"),
            [],
            "running append missing",
        ),
        Mutation(
            "publication-order",
            "wood-on",
            Kind.PUBLISHED,
            ("event",),
            "backend",
            "free before publication",
        ),
        Mutation(
            "callback-order",
            "paused-on",
            Kind.LIMIT_LEAVE,
            ("event",),
            "host_exit",
            "paused callback order",
        ),
        Mutation(
            "paused-tick",
            "paused-on",
            Kind.EXIT,
            ("runtime", "tick"),
            99,
            "paused tick advanced",
        ),
        Mutation(
            "aborted-annotations",
            "raw561-on",
            Kind.JOINED,
            ("annotations",),
            {},
            "aborted annotations",
        ),
    )
    results = {control.name: apply(control, root, output) for control in controls}
    _ = (output / "results.json").write_text(json.dumps(results, indent=2) + "\n")
    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
