# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 -m scripts.depot_replay --inputs INPUTS --artifacts FRESH --oracle ORIGINAL
"""Original depot phases against the owned Rust runtime and complete checkpoints."""

from __future__ import annotations

import argparse
import os
from pathlib import Path

from scripts.depot_controls import controls, resume
from scripts.gameplay_foundations import require_test
from scripts.purchase_replay import rust
from scripts.replay_matrix import ReplayMatrix
from scripts.world_check_support import (
    ROOT,
    Json,
    WorldCheckError,
    at,
    read_json,
    write_json,
)


def compare_live(matrix: ReplayMatrix, native: Path, actual: Path, case: Path) -> None:
    actions = at(read_json(native / "results.json"), ("actions",))
    actual_live = read_json(actual / "live.json")
    match actions, actual_live:
        case list(), list():
            pass
        case _:
            raise WorldCheckError("Missing depot action/live arrays")
    expected: list[Json] = []
    native_phases: list[Json] = []
    rust_phases: list[Json] = []
    for action in actions:
        if at(action, ("op",)) != "command":
            continue
        ordinal = at(action, ("ordinal",))
        metadata = at(action, ("native_metadata",))
        before = at(metadata, ("depot_before",))
        after = before
        match metadata:
            case dict():
                for phase in ["test", "exec", "result"]:
                    if phase not in metadata:
                        continue
                    observed = at(metadata, (phase, "depot"))
                    if phase != "test":
                        after = observed
                    native_phases.append(
                        {"ordinal": ordinal, "phase": phase, "live": observed}
                    )
                    rust_phases.append(
                        {
                            "ordinal": ordinal,
                            "phase": phase,
                            "live": at(
                                actual_live,
                                (
                                    len(expected),
                                    "before" if phase == "test" else "after",
                                ),
                            ),
                        }
                    )
            case _:
                raise WorldCheckError("Missing depot phase metadata")
        expected.append({"ordinal": ordinal, "before": before, "after": after})
    write_json(case / "native-live.json", expected)
    write_json(case / "native-phases.json", native_phases)
    write_json(case / "rust-phases.json", rust_phases)
    matrix.compare(
        case / "native-live.json", actual / "live.json", case / "live-compare"
    )
    matrix.compare(
        case / "native-phases.json", case / "rust-phases.json", case / "phases-compare"
    )


def scenario(matrix: ReplayMatrix, source: Path, plan: Path, case: Path) -> None:
    case.mkdir(parents=True, exist_ok=False)
    empty = case / "empty.json"
    write_json(empty, {"schema_version": 1, "actions": []})
    loaded = matrix.native(source, empty, case / "load")
    native = matrix.native(loaded / "final.sav", plan, case)
    for extension in ["world.json", "derived.json"]:
        matrix.compare(
            loaded / f"final.{extension}",
            native / f"initial.{extension}",
            case / f"canonical-{extension}",
        )
    actual = rust(loaded / "final.sav", plan, case)
    require_test(
        (case / "rust-command/stdout.log").read_text(),
        "runtime::purchase_native::run_native_purchase_sequence",
    )
    matrix.compare_outputs(native, actual, case / "compare")
    compare_live(matrix, native, actual, case / "compare")
    reloaded = matrix.native(actual / "final.sav", empty, case / "rust-reload")
    for extension in ["world.json", "derived.json"]:
        matrix.compare(
            actual / f"final.{extension}",
            reloaded / f"initial.{extension}",
            case / f"reload-{extension}",
        )


class Arguments(argparse.Namespace):
    oracle: Path = ROOT / ".reference/snapshot-build/openttd"
    ottd: Path = ROOT / "target/debug/ottd"
    inputs: Path = Path()
    artifacts: Path = Path()
    case: str | None = None


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    _ = parser.add_argument("--oracle", type=Path, default=Arguments.oracle)
    _ = parser.add_argument("--ottd", type=Path, default=Arguments.ottd)
    _ = parser.add_argument("--inputs", type=Path, required=True)
    _ = parser.add_argument("--artifacts", type=Path, required=True)
    _ = parser.add_argument("--case")
    args = Arguments()
    _ = parser.parse_args(namespace=args)
    inputs, artifacts = args.inputs.resolve(), args.artifacts.resolve()
    artifacts.mkdir(parents=True, exist_ok=False)
    os.environ["OTTD_DEPOT_LIVE"] = "1"
    matrix = ReplayMatrix(
        args.ottd.resolve(), args.oracle.resolve(), args.oracle.resolve(), artifacts
    )
    cases = at(read_json(inputs / "manifest.json"), ("cases",))
    match cases:
        case list():
            pass
        case _:
            raise WorldCheckError("Missing depot case list")
    passed: list[Json] = []
    for name in cases:
        match name:
            case str():
                pass
            case _:
                raise WorldCheckError("Invalid depot case name")
        if args.case is not None and args.case != name:
            continue
        scenario(
            matrix, inputs / f"{name}.sav", inputs / f"{name}.json", artifacts / name
        )
        passed.append(name)
    if not passed:
        raise WorldCheckError("No depot cases executed")
    if args.case is None:
        controls(matrix)
        resume(matrix, inputs, scenario)
    write_json(artifacts / "summary.json", {"passed": passed})
    print(f"PASS {len(passed)} original depot command scenarios")


if __name__ == "__main__":
    main()
