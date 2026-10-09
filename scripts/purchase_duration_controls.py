from __future__ import annotations

from scripts.purchase_creation import common_path, integer
from scripts.purchase_saved_state import command
from scripts.replay_matrix import ReplayMatrix
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
    "loaded-duration",
    "fresh-field",
    "rust-duration",
    "estimate-creation",
    "failed-creation",
    "mismatched-creation",
    "unproven-creation",
    "early-checkpoint",
    "prefix-loaded-duration",
    "reset-action",
    "tick-action",
    "native-duration-range",
    "duplicate-creation",
    "fresh-orders",
    "native-duration-bool",
    "unexpected-rust-metadata",
)


def controls(matrix: ReplayMatrix) -> None:
    for name in CONTROLS:
        source = matrix.artifacts / (
            "split/suffix" if name == "prefix-loaded-duration" else "temperate-original"
        )
        plan_path = (
            matrix.artifacts / "split/suffix.json"
            if name == "prefix-loaded-duration"
            else source / "actions.json"
        )
        directory = matrix.artifacts / "duration-controls" / name
        directory.mkdir(parents=True)
        native, rust = directory / "native", directory / "rust"
        native.mkdir()
        rust.mkdir()
        plan = read_json(plan_path)
        expected_results = read_json(source / "native/results.json")
        actual_results = read_json(source / "rust/results.json")
        expected = read_json(source / "native/final.world.json")
        actual = read_json(source / "rust/final.world.json")
        vehicle = integer(
            at(
                read_json(matrix.artifacts / "temperate-original/native/results.json"),
                ("actions", 2, "receipt", "returns", "exec", "vehicle"),
            )
        )
        path = (*common_path(str(vehicle)), "round_trip_time")
        label, reason = "final", ""
        match name:
            case "unexpected-rust-metadata":
                replace(actual_results, ("actions", 2, "native_metadata"), {})
                reason = "Saved object keys differ"
            case "loaded-duration" | "prefix-loaded-duration":
                loaded = "9" if name == "loaded-duration" else str(vehicle)
                loaded_path = (*common_path(loaded), "round_trip_time")
                replace(actual, loaded_path, integer(at(actual, loaded_path)) + 1)
                reason = "Saved value differs"
            case "fresh-field":
                field = (*common_path(str(vehicle)), "random_bits")
                replace(actual, field, integer(at(actual, field)) + 1)
                reason = "Saved value differs"
            case "rust-duration":
                replace(expected, path, 1)
                replace(actual, path, 1)
                reason = "Rust fresh round_trip_time must be zero"
            case "estimate-creation":
                replace(plan, ("actions", 2, "request", "mode"), "estimate")
                reason = "Uncommitted purchase execution"
            case "failed-creation" | "unproven-creation":
                for results in (expected_results, actual_results):
                    if name == "failed-creation":
                        replace(
                            results, ("actions", 2, "receipt", "exec", "success"), False
                        )
                    else:
                        replace(results, ("actions", 2, "receipt", "exec"), None)
                reason = "unproven or missing creation IDs"
            case "mismatched-creation":
                replace(
                    actual_results,
                    ("actions", 2, "receipt", "returns", "exec", "vehicle"),
                    vehicle + 1,
                )
                reason = "Saved value differs"
            case "early-checkpoint":
                label = "step-1"
                reason = "unproven or missing creation IDs"
            case "reset-action":
                replace(
                    plan,
                    ("actions", 2, "request", "command", "kind"),
                    "start_stop_vehicle",
                )
                reason = "Unsupported purchase lifetime command"
            case "tick-action":
                for value in (plan, expected_results, actual_results):
                    replace(value, ("actions", 2, "op"), "tick")
                reason = "Unsupported purchase lifetime action"
            case "native-duration-range" | "native-duration-bool":
                replace(
                    expected, path, 2**31 if name == "native-duration-range" else True
                )
                reason = (
                    "signed int32"
                    if name == "native-duration-range"
                    else "Expected integer"
                )
            case "duplicate-creation":
                for results in (expected_results, actual_results):
                    for phase in ("exec", "result"):
                        replace(
                            results,
                            ("actions", 4, "receipt", "returns", phase, "vehicle"),
                            vehicle,
                        )
                reason = "reused purchase vehicle identity"
            case "fresh-orders":
                for value in (expected, actual):
                    replace(value, (*common_path(str(vehicle)), "orders"), 1)
                reason = "Fresh purchase domain changed"
            case _:
                raise WorldCheckError("Unknown duration control")
        for target, value in (
            (directory / "plan.json", plan),
            (native / "results.json", expected_results),
            (rust / "results.json", actual_results),
            (native / "final.world.json", expected),
            (rust / "final.world.json", actual),
        ):
            write_json(target, value)
        for target, original in ((native, source / "native"), (rust, source / "rust")):
            write_json(
                target / "initial.world.json",
                read_json(original / "initial.world.json"),
            )
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
            raise WorldCheckError(f"Duration control failed for wrong reason: {name}")
        assertion: Json = {"rejected": True, "reason": reason}
        write_json(directory / "assertion.json", assertion)
