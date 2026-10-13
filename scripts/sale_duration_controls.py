# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through scripts.sale_matrix.
"""Real CLI rejection witnesses for sale lifetime boundaries."""

from scripts.purchase_creation import common_path, integer, sequence
from scripts.replay_matrix import ReplayMatrix
from scripts.sale_saved_state import command
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    read_json,
    replace,
    run,
    write_json,
)

CONTROLS: tuple[str, ...] = (
    "failed-sale-reuse",
    "estimate-sale-reuse",
    "failed-build",
    "freed-id",
    "old-checkpoint",
    "loaded-suffix",
    "unexpected-metadata",
    "equal-nonzero",
    "fresh-field",
    "duration-bool",
    "duration-range",
    "tick",
    "reset",
    "mismatched-return",
    "nonlive-sale",
)


def controls(matrix: ReplayMatrix) -> None:
    for name in CONTROLS:
        source = matrix.artifacts / (
            "split/suffix" if name == "loaded-suffix" else "reuse"
        )
        plan_path = (
            matrix.artifacts / "split/suffix.json"
            if name == "loaded-suffix"
            else source / "actions.json"
        )
        directory = matrix.artifacts / "duration-controls" / name
        native, rust = directory / "native", directory / "rust"
        native.mkdir(parents=True)
        rust.mkdir()
        plan = read_json(plan_path)
        expected_results = read_json(source / "native/results.json")
        actual_results = read_json(source / "rust/results.json")
        initial_native = read_json(source / "native/initial.world.json")
        initial_rust = read_json(source / "rust/initial.world.json")
        expected = read_json(source / "native/final.world.json")
        actual = read_json(source / "rust/final.world.json")
        path = (*common_path("0"), "round_trip_time")
        label, reason = "final", ""
        match name:
            case "failed-sale-reuse" | "estimate-sale-reuse":
                for value in (expected_results, actual_results):
                    replace(value, ("actions", 3, "receipt", "exec"), None)
                if name == "estimate-sale-reuse":
                    replace(plan, ("actions", 3, "request", "mode"), "estimate")
                reason = "Native before occupancy contradicts sale lifetime"
            case "failed-build":
                for value in (expected_results, actual_results):
                    replace(value, ("actions", 8, "receipt", "exec"), None)
                reason = "unproven or missing creation IDs"
            case "freed-id":
                for value in (plan, expected_results, actual_results):
                    replace(value, ("actions",), sequence(value)[:8])
                reason = "unproven or missing creation IDs"
            case "old-checkpoint":
                label = "initial"
                reason = "unproven or missing creation IDs"
            case "loaded-suffix":
                replace(initial_rust, path, integer(at(initial_rust, path)) + 1)
                reason = "Saved value differs"
            case "unexpected-metadata":
                replace(actual_results, ("actions", 0, "native_metadata"), {})
                reason = "Saved object keys differ"
            case "equal-nonzero":
                for value in (expected, actual):
                    replace(value, path, 48)
                reason = "Rust fresh round_trip_time must be zero"
            case "fresh-field":
                field = (*common_path("0"), "random_bits")
                replace(actual, field, integer(at(actual, field)) + 1)
                reason = "Saved value differs"
            case "duration-bool" | "duration-range":
                replace(expected, path, True if name == "duration-bool" else 2**31)
                reason = (
                    "Expected integer" if name == "duration-bool" else "signed int32"
                )
            case "tick":
                for value in (plan, expected_results, actual_results):
                    replace(value, ("actions", 0, "op"), "tick")
                reason = "Unsupported sale lifetime action"
            case "reset":
                replace(
                    plan,
                    ("actions", 0, "request", "command", "kind"),
                    "start_stop_vehicle",
                )
                reason = "Unsupported sale lifetime command"
            case "mismatched-return":
                replace(
                    actual_results,
                    ("actions", 8, "receipt", "returns", "exec", "vehicle"),
                    7,
                )
                reason = "Saved value differs"
            case "nonlive-sale":
                replace(plan, ("actions", 3, "request", "command", "vehicle"), 900)
                reason = "not a live incarnation"
            case _:
                raise WorldCheckError("Unregistered sale duration control")
        for target, value in (
            (directory / "plan.json", plan),
            (native / "results.json", expected_results),
            (rust / "results.json", actual_results),
            (native / "initial.world.json", initial_native),
            (rust / "initial.world.json", initial_rust),
            (native / "final.world.json", expected),
            (rust / "final.world.json", actual),
        ):
            write_json(target, value)
        log = directory / "compare"
        result = run(
            command(
                directory / "plan.json",
                native,
                rust,
                label,
                native / "final.world.json",
                rust / "final.world.json",
                log / "admitted.json",
            ),
            log,
            1,
        )
        if reason not in result.stderr or (log / "admitted.json").exists():
            raise WorldCheckError(
                f"Sale duration control rejected for wrong reason: {name}"
            )
        assertion: Json = {"rejected": True, "reason": reason}
        write_json(directory / "assertion.json", assertion)
