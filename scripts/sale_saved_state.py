# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 -m scripts.sale_saved_state --help
from __future__ import annotations

import argparse
import sys
from pathlib import Path

from scripts.purchase_creation import common_path, fresh_duration, records
from scripts.replay_matrix import deterministic
from scripts.sale_lifetime import lifetimes
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    read_json,
    run,
    write_json,
)


def compare_fields(
    expected: Json,
    actual: Json,
    admitted: set[tuple[str | int, ...]],
    path: tuple[str | int, ...] = (),
) -> None:
    if path in admitted:
        return
    match expected, actual:
        case dict() as left, dict() as right:
            if left.keys() != right.keys():
                raise WorldCheckError(f"Saved object keys differ: {path}")
            for key in left:
                compare_fields(left[key], right[key], admitted, (*path, key))
        case list() as left, list() as right:
            if len(left) != len(right):
                raise WorldCheckError(f"Saved array lengths differ: {path}")
            for index, (one, two) in enumerate(zip(left, right, strict=True)):
                compare_fields(one, two, admitted, (*path, index))
        case _:
            if type(expected) is not type(actual) or expected != actual:
                raise WorldCheckError(
                    f"Saved value differs: {path}: {expected} != {actual}"
                )


def compare(
    plan: Json,
    native_results: Json,
    rust_results: Json,
    native_initial: Json,
    rust_initial: Json,
    label: str,
    expected: Json,
    actual: Json,
) -> Json:
    compare_fields(native_initial, rust_initial, set())
    compare_fields(deterministic(native_results), rust_results, set())
    lifetime = lifetimes(plan, native_results, native_initial, label)
    eligible = lifetime.fresh
    for world in (expected, actual):
        if records(world).keys() != lifetime.live:
            raise WorldCheckError(
                "Checkpoint contains unproven or missing creation IDs"
            )
    admitted: set[tuple[str | int, ...]] = set()
    ledger: list[Json] = []
    for creation in eligible:
        native_value = fresh_duration(expected, creation)
        if fresh_duration(actual, creation) != 0:
            raise WorldCheckError("Rust fresh round_trip_time must be zero")
        path = (*common_path(creation.vehicle), "round_trip_time")
        admitted.add(path)
        ledger.append(
            {
                "vehicle": int(creation.vehicle),
                "creation_ordinal": creation.ordinal,
                "path": list(path),
                "native_value": native_value,
                "rust_value": 0,
                "differs": native_value != 0,
            }
        )
    compare_fields(expected, actual, admitted)
    return {"schema_version": 1, "checkpoint": label, "fresh_initialization": ledger}


def command(
    plan: Path,
    native: Path,
    rust: Path,
    label: str,
    expected: Path,
    actual: Path,
    ledger: Path,
) -> list[str]:
    return [
        sys.executable,
        "-m",
        "scripts.sale_saved_state",
        "--plan",
        str(plan),
        "--native",
        str(native),
        "--rust",
        str(rust),
        "--label",
        label,
        "--expected",
        str(expected),
        "--actual",
        str(actual),
        "--ledger",
        str(ledger),
    ]


def compare_saved(
    plan: Path,
    native: Path,
    rust: Path,
    label: str,
    expected: Path,
    actual: Path,
    log: Path,
) -> None:
    _ = run(
        command(plan, native, rust, label, expected, actual, log / "admitted.json"), log
    )


class Arguments(argparse.Namespace):
    plan: Path = Path()
    native: Path = Path()
    rust: Path = Path()
    label: str = ""
    expected: Path = Path()
    actual: Path = Path()
    ledger: Path = Path()


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Compare the explicit fresh native duration exception"
    )
    for option in ("plan", "native", "rust", "expected", "actual", "ledger"):
        _ = parser.add_argument(f"--{option}", type=Path, required=True)
    _ = parser.add_argument("--label", required=True)
    args = Arguments()
    _ = parser.parse_args(namespace=args)
    try:
        ledger = compare(
            read_json(args.plan),
            read_json(args.native / "results.json"),
            read_json(args.rust / "results.json"),
            read_json(args.native / "initial.world.json"),
            read_json(args.rust / "initial.world.json"),
            args.label,
            read_json(args.expected),
            read_json(args.actual),
        )
        write_json(args.ledger, ledger)
    except (WorldCheckError, KeyError, IndexError) as error:
        print(f"Sale saved-state comparison rejected: {error}", file=sys.stderr)
        return 1
    print(f"PASS sale saved-state {args.label}; ledger={args.ledger}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
