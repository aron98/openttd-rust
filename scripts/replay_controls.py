# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 -m scripts.check-replay --artifacts FRESH_PATH
"""Independent replay output mutations and retained-evidence failure controls."""

from __future__ import annotations

import copy
import shutil
from dataclasses import dataclass
from pathlib import Path
from scripts.replay_matrix import FIXTURES, ReplayMatrix, deterministic
from scripts.world_check_support import (
    ROOT,
    WorldCheckError,
    at,
    read_json,
    replace,
    run,
    write_json,
)


@dataclass(frozen=True, slots=True)
class Mutation:
    name: str
    source: Path
    path: tuple[str | int, ...]


def value_control(matrix: ReplayMatrix, mutation: Mutation) -> None:
    case = matrix.artifacts / "controls" / mutation.name
    case.mkdir(parents=True)
    original = read_json(mutation.source)
    changed = copy.deepcopy(original)
    value = at(changed, mutation.path)
    if not isinstance(value, int) or isinstance(value, bool):
        raise WorldCheckError(f"Mutation requires integer: {mutation.name}")
    replace(changed, mutation.path, value + 1)
    target = case / "mutated.json"
    write_json(target, changed)
    result = run(
        [str(matrix.cli), "compare", str(mutation.source), str(target)],
        case / "compare",
        1,
    )
    path = "$" + "".join(
        f"[{part}]" if isinstance(part, int) else f".{part}" for part in mutation.path
    )
    if (
        path not in result.stderr
        or str(value) not in result.stderr
        or str(value + 1) not in result.stderr
    ):
        raise WorldCheckError(f"Control failed for wrong path/values: {mutation.name}")
    write_json(
        case / "assertion.json",
        {"path": path, "expected": value, "actual": value + 1, "rejected": True},
    )


def output_controls(matrix: ReplayMatrix) -> None:
    commands = matrix.artifacts / "commands/native"
    ticks = matrix.artifacts / "ticks/native"
    controls = [
        Mutation(
            "wrong-cost",
            commands / "results.json",
            ("actions", 0, "receipt", "result", "cost"),
        ),
        Mutation(
            "wrong-clock",
            ticks / "final.world.json",
            ("chunks", "DATE", "records", "0", "tick_counter"),
        ),
        Mutation(
            "wrong-rng",
            ticks / "final.world.json",
            ("chunks", "DATE", "records", "0", "random_state[0]"),
        ),
        Mutation(
            "wrong-state",
            commands / "final.world.json",
            ("chunks", "PLYR", "records", "0", "money"),
        ),
    ]
    for mutation in controls:
        value_control(matrix, mutation)
    case = matrix.artifacts / "controls/wrong-order"
    case.mkdir(parents=True)
    actions = read_json(FIXTURES / "commands.json")
    first = at(actions, ("actions", 0, "request"))
    second = at(actions, ("actions", 2, "request"))
    replace(actions, ("actions", 0, "request"), second)
    replace(actions, ("actions", 2, "request"), first)
    write_json(case / "reordered.json", actions)
    changed = matrix.rust(
        FIXTURES / "populated-v362.sav", case / "reordered.json", case
    )
    original = case / "native.json"
    write_json(original, deterministic(read_json(commands / "results.json")))
    result = run(
        [str(matrix.cli), "compare", str(original), str(changed / "results.json")],
        case / "compare",
        1,
    )
    if "$.actions[0]" not in result.stderr:
        raise WorldCheckError(
            "Wrong-order control failed outside first reordered action"
        )


def evidence_controls(matrix: ReplayMatrix) -> None:
    original = matrix.artifacts / "ticks/native"
    for name, filename in [
        ("missing-save", "final.sav"),
        ("missing-receipts", "results.json"),
    ]:
        case = matrix.artifacts / "controls" / name
        copied = case / "native"
        _ = shutil.copytree(original, copied)
        (copied / filename).unlink()
        result = run(
            [
                "cmake",
                f"-DRUN_DIR={copied}",
                "-P",
                str(ROOT / "scripts/check-replay-evidence.cmake"),
            ],
            case / "validate",
            1,
        )
        if filename not in result.stderr:
            raise WorldCheckError(
                f"Missing-artifact control failed for wrong reason: {name}"
            )
    case = matrix.artifacts / "controls/stale-directory"
    result = run(
        [
            "cmake",
            f"-DORACLE={matrix.oracle}",
            f"-DRUN_DIR={original}",
            f"-DCONFIG={ROOT}/scripts/reference.cfg",
            f"-DINPUT={FIXTURES}/clear-v362.sav",
            f"-DREPLAY={FIXTURES}/ticks.json",
            "-P",
            str(ROOT / "scripts/check-replay-native.cmake"),
        ],
        case,
        1,
    )
    if "stale evidence" not in result.stderr:
        raise WorldCheckError("Stale-directory control failed for wrong reason")
