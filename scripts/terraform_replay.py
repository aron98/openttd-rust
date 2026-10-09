"""Terraform saved continuation and independent replay failure controls."""

from __future__ import annotations

import copy

from scripts.replay_controls import Mutation, value_control
from scripts.replay_matrix import FIXTURES, ReplayMatrix
from scripts.world_check_support import (
    ROOT,
    WorldCheckError,
    at,
    read_json,
    replace,
    run,
    write_json,
)


def verify_terraform(matrix: ReplayMatrix, selected: list[str]) -> None:
    if "terraform-basic" in selected:
        base = matrix.artifacts / "terraform-basic"
        for mutation in [
            Mutation(
                "terraform-tuple",
                base / "compare/native-results.json",
                ("actions", 0, "receipt", "returns", "result", "tile"),
            ),
            Mutation(
                "terraform-cost",
                base / "compare/native-results.json",
                ("actions", 0, "receipt", "test", "cost"),
            ),
            Mutation(
                "terraform-height",
                base / "native/north.world.json",
                ("chunks", "MAPH", "bytes", 2056),
            ),
        ]:
            value_control(matrix, mutation)
        wrong_order(matrix)
    if "terraform-resume" in selected:
        resumed_execution(matrix)
        original_continuation(matrix)


def wrong_order(matrix: ReplayMatrix) -> None:
    case = matrix.artifacts / "controls/terraform-order"
    case.mkdir(parents=True)
    actions = copy.deepcopy(read_json(FIXTURES / "terraform-basic.json"))
    replace(actions, ("actions", 0, "request", "mode"), "post")
    replace(actions, ("actions", 2, "request", "mode"), "estimate")
    path = case / "actions.json"
    write_json(path, actions)
    actual = matrix.rust(
        matrix.artifacts / "terraform-basic/native/initial.sav", path, case
    )
    result = run(
        [
            str(matrix.cli),
            "compare",
            str(matrix.artifacts / "terraform-basic/compare/native-results.json"),
            str(actual / "results.json"),
        ],
        case / "compare",
        1,
    )
    if "$.actions[0]" not in result.stderr:
        raise WorldCheckError(
            "Wrong terraform order was not rejected at the first command"
        )


def resumed_execution(matrix: ReplayMatrix) -> None:
    case = matrix.artifacts / "terraform-split"
    case.mkdir(parents=True)
    actions = at(read_json(FIXTURES / "terraform-resume.json"), ("actions",))
    if not isinstance(actions, list):
        raise WorldCheckError("Terraform actions must be a list")
    write_json(case / "prefix.json", {"schema_version": 1, "actions": actions[:2]})
    write_json(case / "suffix.json", {"schema_version": 1, "actions": actions[2:]})
    source = matrix.artifacts / "terraform-resume/native/initial.sav"
    native_prefix = matrix.native(source, case / "prefix.json", case / "prefix")
    rust_prefix = case / "prefix/rust"
    _ = run(
        [
            str(matrix.cli),
            "replay-world",
            str(source),
            str(FIXTURES / "terraform-resume.json"),
            str(rust_prefix),
            "--through",
            "1",
        ],
        case / "prefix/rust-command",
    )
    matrix.compare_outputs(native_prefix, rust_prefix, case / "prefix/compare")
    native_suffix = matrix.native(
        native_prefix / "final.sav", case / "suffix.json", case / "suffix"
    )
    rust_suffix = case / "suffix/rust"
    _ = run(
        [
            str(matrix.cli),
            "resume-world",
            str(rust_prefix / "checkpoint.json"),
            str(rust_suffix),
        ],
        case / "suffix/rust-command",
    )
    matrix.compare_outputs(native_suffix, rust_suffix, case / "suffix/compare")
    for engine, directory in [("native", native_suffix), ("rust", rust_suffix)]:
        for extension in ("world.json", "derived.json"):
            matrix.compare(
                matrix.artifacts / f"terraform-resume/{engine}/final.{extension}",
                directory / f"final.{extension}",
                case / f"continuous-{engine}-{extension}",
            )


def original_continuation(matrix: ReplayMatrix) -> None:
    case = matrix.artifacts / "terraform-original-continuation"
    source = matrix.artifacts / "terraform-resume/rust/final.sav"
    output = case / "native"
    _ = run(
        [
            "cmake",
            f"-DORACLE={matrix.original}",
            f"-DRUN_DIR={output}",
            f"-DCONFIG={ROOT}/scripts/reference.cfg",
            f"-DINPUT={source}",
            "-DTICKS=16",
            "-P",
            str(ROOT / "scripts/run-reference.cmake"),
        ],
        case / "command",
    )
    if "[grf:0]" in (output / "stderr.log").read_text():
        raise WorldCheckError(
            "Original terraform continuation reported content warning"
        )
    saved = output / "save/autosave/exit.sav"
    exported = run(
        [str(matrix.cli), "world", str(saved), "--view", "saved"], case / "world-export"
    )
    path = case / "continued.world.json"
    _ = path.write_text(exported.stdout)
    before = read_json(matrix.artifacts / "terraform-resume/rust/final.world.json")
    after = read_json(path)
    tick_path = ("chunks", "DATE", "records", "0", "tick_counter")
    before_tick, after_tick = at(before, tick_path), at(after, tick_path)
    if (
        not isinstance(before_tick, int)
        or not isinstance(after_tick, int)
        or after_tick <= before_tick
    ):
        raise WorldCheckError(
            "Original terraform continuation did not execute unpaused ticks"
        )
    height_path = ("chunks", "MAPH", "bytes", 2056)
    if at(before, height_path) != at(after, height_path):
        raise WorldCheckError("Original continuation changed terraformed corner height")
    write_json(
        case / "witness.json",
        {
            "before_tick": before_tick,
            "after_tick": after_tick,
            "tile": 2056,
            "height_preserved": True,
            "scope": "Original live continuation; no populated Rust tick parity claim",
        },
    )
