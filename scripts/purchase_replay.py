# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 -m scripts.purchase_replay --input SAVE --plan PLAN --artifacts FRESH
"""Original command phases versus one live owned Rust simulation runtime."""

from __future__ import annotations

import argparse
from functools import partial
from pathlib import Path

from scripts.purchase_saved_state import compare_saved
from scripts.replay_matrix import ReplayMatrix
from scripts.world_check_support import (
    ROOT,
    Json,
    WorldCheckError,
    at,
    read_json,
    run,
    write_json,
)


def rust(source: Path, plan: Path, case: Path) -> Path:
    output = case / "rust"
    result = run(
        [
            "env",
            f"PURCHASE_INPUT={source}",
            f"PURCHASE_PLAN={plan}",
            f"PURCHASE_OUTPUT={output}",
            "cargo",
            "test",
            "--locked",
            "-p",
            "ottd-sim",
            "--lib",
            "runtime::purchase_native::run_native_purchase_sequence",
            "--",
            "--ignored",
            "--exact",
        ],
        case / "rust-command",
    )
    if "test result: ok. 1 passed; 0 failed; 0 ignored" not in result.stdout:
        raise WorldCheckError("Purchase runtime harness did not execute exactly once")
    return output


def compare_live(matrix: ReplayMatrix, native: Path, actual: Path, case: Path) -> None:
    actions = at(read_json(native / "results.json"), ("actions",))
    match actions:
        case list():
            pass
        case _:
            raise WorldCheckError("Native purchase actions missing")
    expected: list[Json] = []
    rust_live = read_json(actual / "live.json")
    phases: list[Json] = []
    actual_phases: list[Json] = []
    for action in actions:
        if at(action, ("op",)) != "command":
            continue
        metadata = at(action, ("native_metadata",))
        before = at(metadata, ("purchase_before",))
        after = before
        match metadata:
            case dict():
                for phase in ("exec", "result"):
                    if phase in metadata:
                        after = at(metadata, (phase, "live"))
                for phase in ("test", "exec", "result"):
                    if phase in metadata:
                        phases.append(
                            {
                                "ordinal": at(action, ("ordinal",)),
                                "phase": phase,
                                "live": at(metadata, (phase, "live")),
                            }
                        )
                        actual_phases.append(
                            {
                                "ordinal": at(action, ("ordinal",)),
                                "phase": phase,
                                "live": at(
                                    rust_live,
                                    (
                                        len(expected),
                                        "before" if phase == "test" else "after",
                                    ),
                                ),
                            }
                        )
            case _:
                raise WorldCheckError("Native purchase phase metadata missing")
        expected.append(
            {"ordinal": at(action, ("ordinal",)), "before": before, "after": after}
        )
    write_json(case / "native-live.json", expected)
    matrix.compare(
        case / "native-live.json", actual / "live.json", case / "live-compare"
    )
    write_json(case / "native-live-phases.json", phases)
    write_json(case / "rust-live-phases.json", actual_phases)
    matrix.compare(
        case / "native-live-phases.json",
        case / "rust-live-phases.json",
        case / "live-phases-compare",
    )


def scenario(matrix: ReplayMatrix, source: Path, plan: Path, case: Path) -> None:
    case.mkdir(parents=True, exist_ok=True)
    empty = case / "empty.json"
    write_json(empty, {"schema_version": 1, "actions": []})
    loaded = matrix.native(source, empty, case / "load")
    native = matrix.native(loaded / "final.sav", plan, case)
    for extension in ("world.json", "derived.json"):
        matrix.compare(
            loaded / f"final.{extension}",
            native / f"initial.{extension}",
            case / f"canonical-reload-{extension}",
        )
    actual = rust(loaded / "final.sav", plan, case)
    matrix.compare_outputs(
        native,
        actual,
        case / "compare",
        saved_world_compare=partial(compare_saved, plan, native, actual),
    )
    compare_live(matrix, native, actual, case / "compare")
    reloaded = matrix.native(actual / "final.sav", empty, case / "rust-reload")
    for extension in ("world.json", "derived.json"):
        matrix.compare(
            actual / f"final.{extension}",
            reloaded / f"initial.{extension}",
            case / f"reload-{extension}",
        )


class Arguments(argparse.Namespace):
    oracle: Path = ROOT / ".reference/snapshot-build/openttd"
    ottd: Path = ROOT / "target/debug/ottd"
    input: Path = Path()
    plan: Path = Path()
    artifacts: Path = Path()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    _ = parser.add_argument(
        "--oracle", type=Path, default=ROOT / ".reference/snapshot-build/openttd"
    )
    _ = parser.add_argument("--ottd", type=Path, default=ROOT / "target/debug/ottd")
    for option in ("input", "plan", "artifacts"):
        _ = parser.add_argument(f"--{option}", type=Path, required=True)
    args = Arguments()
    _ = parser.parse_args(namespace=args)
    artifacts = args.artifacts.resolve()
    artifacts.mkdir(parents=True, exist_ok=False)
    matrix = ReplayMatrix(
        args.ottd.resolve(), args.oracle.resolve(), args.oracle.resolve(), artifacts
    )
    scenario(matrix, args.input.resolve(), args.plan.resolve(), artifacts)


if __name__ == "__main__":
    main()
