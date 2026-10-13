"""Persist actual mutated artifacts and require the production comparers to refuse."""

from __future__ import annotations

from collections.abc import Callable
from pathlib import Path

from .compare import checkpoint_words, differences, words
from .native import write
from .physical import CACHE_FIELDS, compare
from .qualify import Qualified
from .value import (
    DECODE,
    EvidenceError,
    Json,
    array,
    field,
    integer,
    require,
    same,
    string,
)


def replace(value: Json, path: tuple[str, ...], replacement: Json) -> Json:
    if not path:
        return replacement
    key, *tail = path
    if isinstance(value, dict):
        require("mutation path missing", condition=key in value)
        return {
            name: replace(child, tuple(tail), replacement) if name == key else child
            for name, child in value.items()
        }
    values = array(value)
    index = int(key)
    require("mutation index missing", condition=0 <= index < len(values))
    return [
        replace(child, tuple(tail), replacement) if i == index else child
        for i, child in enumerate(values)
    ]


def refusal(
    directory: Path, name: str, value: Json, check: Callable[[Json], None]
) -> Json:
    artifact = directory / (name + ".json")
    write(artifact, value)
    reread = DECODE(artifact.read_text())
    try:
        check(reread)
    except EvidenceError as error:
        return {"artifact": artifact.name, "refused": True, "diagnostic": str(error)}
    raise EvidenceError("corrupt artifact admitted: " + name)


def exact(expected: Json, actual: Json) -> None:
    require("saved gameplay differs", condition=same(expected, actual))


def run(output: Path, case: Qualified) -> None:
    stem = case.case.name.replace("-", "_")
    directory = output / (stem + "_corruption")
    directory.mkdir()
    world = DECODE((case.traced / "initial.world.json").read_text())
    date = ("chunks", "DATE", "records", "0", "random_state[0]")
    current = world
    for key in date:
        current = field(current, key)
    vehicle = ("chunks", "VEHS", "records", str(case.subject), "roadveh", "0")
    mutations: tuple[tuple[str, tuple[str, ...], Json], ...] = (
        ("saved_rng", date, integer(current) ^ 1),
        ("saved_path", (*vehicle, "path"), [{"trackdir": 0, "tile": 670}]),
        ("vehicle_frame", (*vehicle, "frame"), 255),
    )
    receipts: list[Json] = []
    exact(world, world)
    for name, path, value in mutations:
        receipts.append(
            refusal(
                directory,
                name,
                replace(world, path, value),
                lambda actual: exact(world, actual),
            )
        )
    movement = DECODE((case.traced / "initial.movement.json").read_text())
    physical = DECODE(
        (output / (stem + "_runtime") / "initial.physical.json").read_text()
    )
    compare(movement, physical, int(case.subject))
    for name in sorted(CACHE_FIELDS):
        current = field(array(field(physical, "road_caches"))[0], name)
        replacement = 0 if current is None else integer(current) ^ 1
        mutated = replace(physical, ("road_caches", "0", name), replacement)
        receipts.append(
            refusal(
                directory,
                "physical_" + name,
                mutated,
                lambda actual: compare(movement, actual, int(case.subject)),
            )
        )
    occupancy = field(physical, "single_road_tile_occupancy")
    for name in ("bucket", "vehicle"):
        mutated = replace(
            physical,
            ("single_road_tile_occupancy", name),
            integer(field(occupancy, name)) ^ 1,
        )
        receipts.append(
            refusal(
                directory,
                "spatial_" + name,
                mutated,
                lambda actual: compare(movement, actual, int(case.subject)),
            )
        )
    runtime = DECODE((case.traced / "initial.runtime.json").read_text())
    initial = checkpoint_words(case.traced, "initial")
    for word in range(2):
        changed = replace(
            runtime, ("after_save", "interactive_random", str(word)), initial[word] ^ 1
        )

        def unchanged(actual: Json) -> None:
            require(
                "process input evolution",
                condition=words(
                    field(field(actual, "after_save"), "interactive_random")
                )
                == initial,
            )

        receipts.append(
            refusal(directory, f"interactive_evolution_{word}", changed, unchanged)
        )
    changed = replace(runtime, ("after_save", "current_company"), 254)

    def runtime_equal(actual: Json) -> None:
        require(
            "runtime noninput difference",
            condition=not differences(runtime, actual, ("runtime",)),
        )

    receipts.append(refusal(directory, "runtime_company", changed, runtime_equal))
    write(
        directory / "summary.json",
        {"case": case.case.name, "actual_serialized_refusals": receipts},
    )


def verify(output: Path, case: Qualified) -> None:
    stem = case.case.name.replace("-", "_")
    directory = output / (stem + "_corruption")
    summary = DECODE((directory / "summary.json").read_text())
    names = {
        "saved_rng",
        "saved_path",
        "vehicle_frame",
        "runtime_company",
        "interactive_evolution_0",
        "interactive_evolution_1",
        "spatial_bucket",
        "spatial_vehicle",
    } | {"physical_" + name for name in CACHE_FIELDS}
    rows = array(field(summary, "actual_serialized_refusals"))
    require(
        "complete serialized mutation roster",
        condition=len(rows) == len(names)
        and {string(field(row, "artifact")) for row in rows}
        == {name + ".json" for name in names},
    )
    require(
        "exact corruption membership",
        condition={path.name for path in directory.iterdir()}
        == {"summary.json", *(name + ".json" for name in names)},
    )
    world = DECODE((case.traced / "initial.world.json").read_text())
    movement = DECODE((case.traced / "initial.movement.json").read_text())
    runtime = DECODE((case.traced / "initial.runtime.json").read_text())
    initial = checkpoint_words(case.traced, "initial")
    for name in sorted(names):
        actual = DECODE((directory / (name + ".json")).read_text())
        try:
            if name.startswith(("physical_", "spatial_")):
                compare(movement, actual, int(case.subject))
            elif name.startswith("interactive_evolution_"):
                require(
                    "process input evolution",
                    condition=words(
                        field(field(actual, "after_save"), "interactive_random")
                    )
                    == initial,
                )
            elif name == "runtime_company":
                require(
                    "runtime noninput difference",
                    condition=not differences(runtime, actual, ("runtime",)),
                )
            else:
                exact(world, actual)
        except EvidenceError:
            continue
        raise EvidenceError("serialized corruption now admitted: " + name)
