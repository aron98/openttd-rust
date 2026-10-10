# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 -m scripts.sale_evidence DIRECTORY
"""Exact membership and semantic admission for fresh road sale evidence."""

import sys
from pathlib import Path

from scripts.gameplay_foundations import require_test
from scripts.purchase_evidence import archive_files, paths_from_layout, validate_paths
from scripts.replay_matrix import checkpoint_labels, deterministic
from scripts.sale_duration_controls import CONTROLS as DURATION_CONTROLS
from scripts.sale_provenance import verify
from scripts.sale_saved_state import compare, compare_fields
from scripts.world_check_support import WorldCheckError, at, read_json

CASES = (
    "temperate-original",
    "temperate-realistic",
    "arctic",
    "tropic",
    "toyland",
    "reuse",
    "named-survivor",
    "legacy",
    "crashed",
    "moving",
    "unstopped",
    "wrong-state",
    "outside-depot",
    "nonowner",
    "pause",
    "negative-value",
    "minimum-value",
    "zero-value",
    "insufficient",
    "missing",
    "zero-location",
    "invalid-location",
)
CONTROLS = (
    "refund",
    "expense",
    "pool-items",
    "pool-cursor",
    "released-unit",
    "survivor-cache",
    "all-group",
    "default-group",
    "engine-count",
    "rng",
    "serializer-context",
)


def validate_summary(root: Path) -> None:
    summary = read_json(root / "summary.json")
    if summary != {
        "passed": True,
        "cases": list(CASES),
        "original_cleanup_commands": True,
    }:
        raise WorldCheckError("Incomplete sale scenario membership")


def validate_cases(root: Path) -> None:
    validate_summary(root)
    commands = 0
    executed = 0
    engines: set[int] = set()
    for name in (*CASES, "split/prefix", "split/suffix"):
        case = root / name
        plan = (
            root / f"{name}.json"
            if name.startswith("split/")
            else case / "actions.json"
        )
        native = read_json(case / "native/results.json")
        rust = read_json(case / "rust/results.json")
        compare_fields(deterministic(native), rust, set())
        for extension in ("world.json", "derived.json"):
            compare_fields(
                read_json(case / f"load/native/final.{extension}"),
                read_json(case / f"native/initial.{extension}"),
                set(),
            )
            compare_fields(
                read_json(case / f"rust/final.{extension}"),
                read_json(case / f"reload/native/initial.{extension}"),
                set(),
            )
        compare_fields(
            read_json(case / "compare/native-sale-live.json"),
            read_json(case / "rust/sale-live.json"),
            set(),
        )
        compare_fields(
            read_json(case / "compare/native-sale-phases.json"),
            read_json(case / "compare/rust-sale-phases.json"),
            set(),
        )
        for label in checkpoint_labels(native):
            compare_fields(
                read_json(case / f"native/{label}.derived.json"),
                read_json(case / f"rust/{label}.derived.json"),
                set(),
            )
            compare_fields(
                deterministic(read_json(case / f"native/{label}.runtime.json")),
                read_json(case / f"rust/{label}.runtime.json"),
                set(),
            )
            expected = read_json(case / f"native/{label}.world.json")
            actual = read_json(case / f"rust/{label}.world.json")
            compare_fields(
                actual, read_json(case / f"compare/{label}.decoded-save.json"), set()
            )
            ledger = compare(
                read_json(plan),
                native,
                rust,
                read_json(case / "native/initial.world.json"),
                read_json(case / "rust/initial.world.json"),
                label,
                expected,
                actual,
            )
            for comparison in ("world.json", "save"):
                compare_fields(
                    ledger,
                    read_json(
                        case / f"compare/{label}-{comparison}-compare/admitted.json"
                    ),
                    set(),
                )
        if name not in CASES:
            continue
        actions = at(read_json(plan), ("actions",))
        observed = at(native, ("actions",))
        match actions, observed:
            case list(), list() if len(actions) == len(observed):
                for action, result in zip(actions, observed, strict=True):
                    if at(action, ("op",)) != "command":
                        continue
                    commands += 1
                    execution = at(result, ("receipt", "exec"))
                    if execution is not None and at(execution, ("success",)) is True:
                        executed += 1
                        if (
                            at(action, ("request", "command", "kind"))
                            == "build_vehicle"
                        ):
                            engine = at(action, ("request", "command", "engine"))
                            match engine:
                                case int() if not isinstance(engine, bool):
                                    engines.add(engine)
                                case _:
                                    raise WorldCheckError("Invalid sale engine")
            case _:
                raise WorldCheckError("Sale action membership differs")
    if (commands, executed, len(engines)) != (571, 370, 88):
        raise WorldCheckError("Incomplete sale action coverage")


def validate_receipts(root: Path, paths: list[Path]) -> None:
    failures = {f"controls/{name}/compare/process.json" for name in CONTROLS}
    failures.update(
        f"duration-controls/{name}/compare/process.json" for name in DURATION_CONTROLS
    )
    receipts = [path for path in paths if path.name == "process.json"]
    if len(receipts) != 586:
        raise WorldCheckError(f"Incorrect sale receipt count: {len(receipts)}")
    for path in receipts:
        expected = int(str(path.relative_to(root)) in failures)
        if read_json(path) != {"returncode": expected, "expected": expected}:
            raise WorldCheckError(f"Sale process failed: {path}")
    for directory, test in (
        ("cleanup-helper", "cleanup::prepare_sale_cleanup"),
        ("inputs-helper", "inputs::prepare_sale_inputs"),
    ):
        require_test((root / directory / "stdout.log").read_text(), test)
    for name in (*CASES, "split/prefix", "split/suffix"):
        require_test(
            (root / name / "rust-command/stdout.log").read_text(),
            "runtime::purchase_native::run_native_purchase_sequence",
        )
    for name in CONTROLS:
        if (
            at(read_json(root / "controls" / name / "assertion.json"), ("rejected",))
            is not True
        ):
            raise WorldCheckError("Sale corruption not rejected")

    for name in DURATION_CONTROLS:
        if (
            at(
                read_json(root / "duration-controls" / name / "assertion.json"),
                ("rejected",),
            )
            is not True
        ):
            raise WorldCheckError("Sale duration corruption not rejected")


def package(directory: Path) -> None:
    root = directory / "results"
    expected = paths_from_layout(
        read_json(Path(__file__).with_name("sale-evidence-layout.json"))
    )
    paths = validate_paths(root, expected)
    validate_cases(root)
    validate_receipts(root, paths)
    verify(directory, (*CASES, "split/prefix", "split/suffix"))
    archive_files(directory, paths)


if __name__ == "__main__":
    package(Path(sys.argv[1]).resolve())
