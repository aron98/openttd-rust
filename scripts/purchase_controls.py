# SPDX-License-Identifier: GPL-2.0-only
"""Purchase continuation and comparator rejection witnesses."""

from __future__ import annotations

from scripts.purchase_replay import scenario
from scripts.replay_controls import Mutation, value_control
from scripts.replay_matrix import ReplayMatrix
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


def coverage(case: Json, results: Json) -> None:
    name = at(case, ("name",))
    actions = at(results, ("actions",))
    match actions:
        case list():
            posted = sum(
                at(action, ("receipt", "exec")) is not None
                and at(action, ("receipt", "exec", "success")) is True
                for action in actions
                if at(action, ("op",)) == "command"
            )
        case _:
            raise WorldCheckError("Missing native purchase actions")
    match name:
        case (
            "temperate-original"
            | "temperate-realistic"
            | "arctic"
            | "tropic"
            | "toyland"
        ):
            engines = at(case, ("engines",))
            match engines:
                case list():
                    expected = 2 + len(engines)
                case _:
                    raise WorldCheckError("Missing native engine coverage")
        case "limit" | "money" | "nonowner" | "unavailable" | "wrong-depot" | "pause":
            expected = 0
        case "exact-money":
            expected = 1
        case "dynamic" | "legacy-paid" | "errors":
            expected = 2
        case _:
            raise WorldCheckError("Unregistered native purchase coverage")
    if posted != expected:
        raise WorldCheckError(
            f"Native purchase coverage {name}: expected {expected}, got {posted}"
        )


def controls(matrix: ReplayMatrix) -> None:
    case = matrix.artifacts / "temperate-original"
    results = case / "compare/native-results.json"
    world = case / "native/final.world.json"
    live = case / "compare/native-live.json"
    vehicle = at(
        read_json(results), ("actions", 2, "receipt", "returns", "exec", "vehicle")
    )
    match vehicle:
        case int():
            pass
        case _:
            raise WorldCheckError("Purchase vehicle ID missing")
    common = ("chunks", "VEHS", "records", str(vehicle), "roadveh", 0, "common", 0)
    roads = at(read_json(live), (1, "after", "road"))
    match roads:
        case list():
            index = next(
                (i for i, row in enumerate(roads) if at(row, ("id",)) == vehicle), None
            )
        case _:
            raise WorldCheckError("Missing immediate road caches")
    if index is None:
        raise WorldCheckError("Missing created vehicle cache")
    for mutation in (
        Mutation(
            "vehicle-id",
            results,
            ("actions", 2, "receipt", "returns", "exec", "vehicle"),
        ),
        Mutation(
            "cargo-array",
            results,
            ("actions", 2, "receipt", "returns", "exec", "cargo_capacities", 63),
        ),
        Mutation("cost", results, ("actions", 2, "receipt", "exec", "cost")),
        Mutation("expense", results, ("actions", 2, "receipt", "exec", "expenses")),
        Mutation("rng", world, ("chunks", "DATE", "records", "0", "random_state[0]")),
        Mutation("vehicle-random", world, (*common, "random_bits")),
        Mutation("service-date", world, (*common, "date_of_last_service")),
        Mutation("orders-reference", world, (*common, "orders")),
        Mutation("pool-cursor", live, (1, "after", "pool", "first_free")),
        Mutation("unit-number", live, (1, "after", "units", 0, "next")),
        Mutation("creation-power", live, (1, "after", "road", index, "power")),
    ):
        value_control(matrix, mutation)


def resume(matrix: ReplayMatrix) -> None:
    source = matrix.artifacts / "temperate-original"
    actions = at(read_json(source / "actions.json"), ("actions",))
    match actions:
        case list():
            pass
        case _:
            raise WorldCheckError("Purchase actions missing")
    directory = matrix.artifacts / "split"
    directory.mkdir()
    write_json(directory / "prefix.json", {"schema_version": 1, "actions": actions[:4]})
    write_json(directory / "suffix.json", {"schema_version": 1, "actions": actions[4:]})
    scenario(
        matrix,
        source / "load/native/final.sav",
        directory / "prefix.json",
        directory / "prefix",
    )
    scenario(
        matrix,
        directory / "prefix/rust/final.sav",
        directory / "suffix.json",
        directory / "suffix",
    )
    for extension in ("world.json", "derived.json"):
        matrix.compare(
            source / f"rust/final.{extension}",
            directory / f"suffix/rust/final.{extension}",
            directory / f"continuous-{extension}",
        )
