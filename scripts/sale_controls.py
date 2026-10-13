# SPDX-License-Identifier: GPL-2.0-only
"""Sale comparator rejection and command-only save/resume witnesses."""

from __future__ import annotations

from scripts.replay_controls import Mutation, value_control
from scripts.replay_matrix import ReplayMatrix
from scripts.sale_replay import scenario
from scripts.world_check_support import WorldCheckError, at, read_json, write_json


def controls(matrix: ReplayMatrix) -> None:
    reuse = matrix.artifacts / "reuse"
    live = reuse / "compare/native-sale-live.json"
    results = reuse / "compare/native-results.json"
    legacy = matrix.artifacts / "legacy/native/final.world.json"
    named = matrix.artifacts / "named-survivor/compare/native-sale-live.json"
    groups = (1, "after", "groups", "0")
    engine_counts = at(read_json(named), (*groups, "all", "engines"))
    match engine_counts:
        case dict():
            engine = next((key for key, count in engine_counts.items() if count), None)
        case _:
            raise WorldCheckError("Missing original group engine counts")
    if engine is None:
        raise WorldCheckError("Named survivor has no counted engine")
    for mutation in (
        Mutation("refund", results, ("actions", 3, "receipt", "exec", "cost")),
        Mutation("expense", results, ("actions", 3, "receipt", "exec", "expenses")),
        Mutation("pool-items", live, (3, "after", "vehicle", "pool", "items")),
        Mutation("pool-cursor", live, (3, "after", "vehicle", "pool", "first_free")),
        Mutation("released-unit", live, (3, "after", "vehicle", "units", 0, "next")),
        Mutation("survivor-cache", live, (3, "after", "vehicle", "road", 0, "power")),
        Mutation("all-group", named, (*groups, "all", "vehicles")),
        Mutation("default-group", named, (*groups, "default_group", "vehicles")),
        Mutation("engine-count", named, (*groups, "all", "engines", engine)),
        Mutation("rng", legacy, ("chunks", "DATE", "records", "0", "random_state[0]")),
        Mutation(
            "serializer-context",
            legacy,
            (
                "chunks",
                "VEHS",
                "records",
                "0",
                "roadveh",
                0,
                "common",
                0,
                "cargo_paid_for",
            ),
        ),
    ):
        value_control(matrix, mutation)


def resume(matrix: ReplayMatrix) -> None:
    source = matrix.artifacts / "reuse"
    actions = at(read_json(source / "actions.json"), ("actions",))
    match actions:
        case list():
            pass
        case _:
            raise WorldCheckError("Sale continuation actions missing")
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
