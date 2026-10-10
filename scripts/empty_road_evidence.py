from __future__ import annotations

from pathlib import Path
from typing import Final

from scripts.grf_control_evidence import sequence, text
from scripts.language_ci_compare import compare, mapping, number
from scripts.world_check_support import Json, WorldCheckError, at, read_json

RUNTIME: Final = frozenset(
    {
        "calendar_date",
        "calendar_fract",
        "calendar_month",
        "calendar_year",
        "economy_date",
        "economy_fract",
        "economy_month",
        "economy_year",
        "pause",
        "random",
        "tick",
    }
)
HOST: Final = frozenset({"interactive_random", "current_company"})


def runtime(value: Json) -> Json:
    fields = mapping(value)
    if frozenset(fields) != RUNTIME | HOST:
        raise WorldCheckError("Empty-road native runtime keys differ")
    for key, item in fields.items():
        if key in {"random", "interactive_random"}:
            values = sequence(item)
            if len(values) != 2:
                raise WorldCheckError("Empty-road RNG width differs")
            for word in values:
                if number(word) > 0xFFFFFFFF:
                    raise WorldCheckError("Empty-road RNG word overflow")
        else:
            _ = number(item)
    return {key: value for key, value in fields.items() if key in RUNTIME}


def membership(actual: list[str], expected: list[str]) -> None:
    if not expected or len(set(expected)) != len(expected):
        raise WorldCheckError("Empty-road membership is zero or duplicate")
    if actual != expected:
        raise WorldCheckError("Empty-road membership differs")


def labels(protocol: Json) -> list[str]:
    result = ["initial"]
    result.extend(
        text(at(action, ("label",)))
        for action in sequence(at(protocol, ("actions",)))
        if at(action, ("op",)) == "checkpoint"
    )
    result.append("final")
    membership(result, result)
    return result


def trace(protocol: Json, result: Json, *, native: bool) -> None:
    membership(sorted(mapping(result)), ["actions", "checkpoints", "schema_version"])
    compare(at(result, ("schema_version",)), 1)
    actions = sequence(at(result, ("actions",)))
    requests = sequence(at(protocol, ("actions",)))
    if not requests or len(actions) != len(requests):
        raise WorldCheckError("Empty-road action membership differs")
    for index, (action, request) in enumerate(zip(actions, requests, strict=True)):
        compare(at(action, ("ordinal",)), index)
        compare(at(request, ("ordinal",)), index)
        compare(at(action, ("op",)), at(request, ("op",)))
        action_keys(action, request)
        before, after = at(action, ("before",)), at(action, ("after",))
        for value in (before, after):
            if native:
                _ = runtime(value)
            else:
                membership(sorted(mapping(value)), sorted(RUNTIME))
            compare(at(value, ("pause",)), 0)
        compare(at(before, ("random",)), at(after, ("random",)))
        if native:
            for key in HOST:
                compare(at(before, (key,)), at(after, (key,)))
        if index:
            compare(at(actions[index - 1], ("after",)), before)
        increment = (
            number(at(request, ("count",))) if at(request, ("op",)) == "tick" else 0
        )
        compare(at(after, ("tick",)), number(at(before, ("tick",))) + increment)
    checkpoints = sequence(at(result, ("checkpoints",)))
    membership([text(at(item, ("label",))) for item in checkpoints], labels(protocol))
    for item in checkpoints:
        membership(sorted(mapping(item)), ["label", "runtime"])
        if native:
            _ = runtime(at(item, ("runtime",)))
    checkpoint_links(protocol, actions, checkpoints)


def checkpoint_links(
    protocol: Json, actions: list[Json], checkpoints: list[Json]
) -> None:
    compare(at(actions[0], ("before",)), at(checkpoints[0], ("runtime",)))
    compare(at(actions[-1], ("after",)), at(checkpoints[-1], ("runtime",)))
    for action, request in zip(
        actions, sequence(at(protocol, ("actions",))), strict=True
    ):
        if at(request, ("op",)) == "checkpoint":
            for checkpoint in checkpoints:
                if at(checkpoint, ("label",)) == at(request, ("label",)):
                    compare(at(action, ("after",)), at(checkpoint, ("runtime",)))


def action_keys(action: Json, request: Json) -> None:
    keys = {"ordinal", "op", "before", "after"}
    if at(request, ("op",)) == "command":
        keys.update(("receipt", "native_metadata"))
        if at(action, ("receipt", "posted")) is not True:
            raise WorldCheckError("Original empty-road command was not posted")
        compare(at(action, ("receipt", "gate")), None)
        membership(
            sorted(mapping(at(action, ("native_metadata",)))),
            ["exec", "result", "test"],
        )
        for phase in ("test", "exec", "result"):
            if at(action, ("receipt", phase, "success")) is not True:
                raise WorldCheckError("Original empty-road construction failed")
            compare(at(action, ("receipt", phase, "error")), None)
            compare(
                at(action, ("native_metadata", phase)),
                {"error_id": 65535, "extra_error_id": 65535, "owner": 255},
            )
    membership(sorted(mapping(action)), sorted(keys))


def compare_trace(protocol: Json, native: Json, rust: Json) -> None:
    trace(protocol, native, native=True)
    trace(protocol, rust, native=False)
    for left, right in zip(
        sequence(at(native, ("actions",))),
        sequence(at(rust, ("actions",))),
        strict=True,
    ):
        for phase in ("before", "after"):
            compare(runtime(at(left, (phase,))), at(right, (phase,)))
    for left, right in zip(
        sequence(at(native, ("checkpoints",))),
        sequence(at(rust, ("checkpoints",))),
        strict=True,
    ):
        compare(runtime(at(left, ("runtime",))), at(right, ("runtime",)))


def pair(protocol: Json, native: Path, rust: Path) -> None:
    compare_trace(
        protocol, read_json(native / "results.json"), read_json(rust / "results.json")
    )
    expected = labels(protocol)
    for directory in (native, rust):
        membership(
            sorted(path.stem for path in directory.glob("*.sav")), sorted(expected)
        )
    for label in expected:
        for view in ("world", "derived"):
            compare(
                read_json(native / f"{label}.{view}.json"),
                read_json(rust / f"{label}.{view}.json"),
            )
        left = read_json(native / f"{label}.runtime.json")
        right = read_json(rust / f"{label}.runtime.json")
        membership(sorted(mapping(left)), ["after_save", "before_save"])
        membership(sorted(mapping(right)), ["after_save", "before_save"])
        compare(at(left, ("before_save",)), at(left, ("after_save",)))
        for phase in ("before_save", "after_save"):
            compare(runtime(at(left, (phase,))), at(right, (phase,)))
        for directory, phases in ((native, left), (rust, right)):
            value = at(phases, ("before_save",))
            saved_anchor(read_json(directory / f"{label}.world.json"), value)
            checkpoints = sequence(
                at(read_json(directory / "results.json"), ("checkpoints",))
            )
            for item in checkpoints:
                if at(item, ("label",)) == label:
                    compare(at(item, ("runtime",)), value)


def saved_anchor(world: Json, value: Json) -> None:
    date = at(world, ("chunks", "DATE", "records", "0"))
    for saved, observed in (
        ("tick_counter", "tick"),
        ("date", "calendar_date"),
        ("date_fract", "calendar_fract"),
        ("economy_date", "economy_date"),
        ("economy_date_fract", "economy_fract"),
        ("pause_mode", "pause"),
    ):
        compare(at(date, (saved,)), at(value, (observed,)))
    compare(
        [at(date, ("random_state[0]",)), at(date, ("random_state[1]",))],
        at(value, ("random",)),
    )


def road(world: Json, tile: int) -> Json:
    return at(world, ("chunks", "MAPE", "bytes", tile))


def natural_visits(native: Path) -> None:
    initial = read_json(native / "build/final.world.json")
    before = read_json(native / "ticks/before-road.world.json")
    after = read_json(native / "ticks/after-road.world.json")
    compare(road(initial, 649), 0)
    compare(road(before, 649), 0)
    compare(road(after, 649), 8)
    for case in ("ticks", "stable"):
        world = read_json(native / case / "final.world.json")
        compare(at(world, ("chunks", "DATE", "records", "0", "cur_tileloop_tile")), 1)
        for tile in range(648, 653):
            compare(road(world, tile), 8)
    for name in (
        "MAPT",
        "MAPH",
        "MAPO",
        "MAP2",
        "M3LO",
        "M3HI",
        "MAP5",
        "MAPE",
        "MAP7",
        "MAP8",
    ):
        width = 2 if name in {"MAP2", "MAP8"} else 1
        left = sequence(
            at(
                read_json(native / "ticks/before-depot.world.json"),
                ("chunks", name, "bytes"),
            )
        )
        right = sequence(
            at(
                read_json(native / "ticks/after-depot.world.json"),
                ("chunks", name, "bytes"),
            )
        )
        compare(
            list(left[653 * width : 654 * width]),
            list(right[653 * width : 654 * width]),
        )
