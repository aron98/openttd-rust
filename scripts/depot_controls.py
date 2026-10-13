# SPDX-License-Identifier: GPL-2.0-only
from collections.abc import Callable
from pathlib import Path

from scripts.replay_controls import Mutation, value_control
from scripts.replay_matrix import ReplayMatrix
from scripts.world_check_support import WorldCheckError, at, read_json, write_json


def controls(matrix: ReplayMatrix) -> None:
    flat = matrix.artifacts / "slope-0-enabled-0-dir-0"
    nested = matrix.artifacts / "different-depot-type-sloped"
    occupied = matrix.artifacts / "occupied-max-corner"
    initial = at(
        read_json(flat / "native/initial.world.json"), ("chunks", "DEPT", "records")
    )
    final = at(
        read_json(flat / "native/final.world.json"), ("chunks", "DEPT", "records")
    )
    match initial, final:
        case dict(), dict():
            added = set(final) - set(initial)
        case _:
            raise WorldCheckError("Missing original depot records")
    if len(added) != 1:
        raise WorldCheckError("Original flat case did not create exactly one depot")
    depot = added.pop()
    for mutation in [
        Mutation(
            "depot-cursor",
            flat / "compare/native-live.json",
            (1, "after", "depot", "pool", "first_free"),
        ),
        Mutation(
            "depot-infrastructure",
            flat / "compare/native-live.json",
            (1, "after", "depot", "road", "0", 0),
        ),
        Mutation(
            "depot-name-number",
            flat / "native/final.world.json",
            ("chunks", "DEPT", "records", depot, "town_cn"),
        ),
        Mutation(
            "nested-expense",
            nested / "compare/native-results.json",
            ("actions", 0, "receipt", "result", "expenses"),
        ),
        Mutation(
            "nested-foundation",
            nested / "compare/native-results.json",
            ("actions", 0, "receipt", "result", "cost"),
        ),
        Mutation(
            "occupied-cache",
            occupied / "compare/native-live.json",
            (0, "after", "vehicles", "road", 0, "max_speed"),
        ),
        Mutation(
            "construction-rng",
            flat / "native/final.world.json",
            ("chunks", "DATE", "records", "0", "random_state[0]"),
        ),
    ]:
        value_control(matrix, mutation)


def resume(
    matrix: ReplayMatrix,
    inputs: Path,
    scenario: Callable[[ReplayMatrix, Path, Path, Path], None],
) -> None:
    name = "slope-0-enabled-0-dir-0"
    plan = read_json(inputs / f"{name}.json")
    actions = at(plan, ("actions",))
    match actions:
        case list() if len(actions) == 8:
            pass
        case _:
            raise WorldCheckError("Unexpected depot continuation plan")
    output = matrix.artifacts / "continuation"
    output.mkdir()
    prefix = output / "prefix.json"
    suffix = output / "suffix.json"
    write_json(prefix, {"schema_version": 1, "actions": actions[:4]})
    write_json(suffix, {"schema_version": 1, "actions": actions[4:]})
    scenario(matrix, inputs / f"{name}.sav", prefix, output / "prefix")
    scenario(matrix, output / "prefix/rust/final.sav", suffix, output / "suffix")
    for extension in ["world.json", "derived.json"]:
        matrix.compare(
            matrix.artifacts / name / f"native/final.{extension}",
            output / f"suffix/native/final.{extension}",
            output / f"continuous-{extension}",
        )
