# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 -m scripts.owned_restore_saved_state --help
from __future__ import annotations

import argparse
import sys
from dataclasses import dataclass
from pathlib import Path

from scripts.owned_restore_lifetime import Creation, lifetimes
from scripts.owned_restore_projection import project
from scripts.purchase_creation import common_path, integer, records
from scripts.sale_saved_state import compare_fields
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


def duration(world: Json, creation: Creation) -> int:
    path = common_path(creation.vehicle)
    common = at(world, path)
    if at(world, (*path[:-2], "state")) != 254:
        raise WorldCheckError("Restore creation is not in its road depot")
    fields = {
        "orders": creation.orders,
        "cur_real_order_index": creation.real,
        "cur_implicit_order_index": creation.implicit,
        "next": 0,
        "next_shared": 0,
        "cur_speed": 0,
        "tick_counter": 0,
        "current_order.type": 0,
        "current_order.flags": 0,
        "depot_unbunching_last_departure": 0,
        "depot_unbunching_next_departure": 0,
        "subtype": 1,
        "vehstatus": 11,
        "group_id": 65534,
    }
    for field, expected in fields.items():
        if integer(at(common, (field,))) != expected:
            raise WorldCheckError(f"Restore fresh domain changed: {field}")
    result = integer(at(common, ("round_trip_time",)))
    if not -(2**31) <= result < 2**31:
        raise WorldCheckError("Restore fresh duration is not int32")
    return result


@dataclass(frozen=True, slots=True)
class Comparison:
    plan: Json
    native: Json
    rust: Json
    initial_native: Json
    initial_rust: Json
    label: str
    expected: Json
    actual: Json


def compare(proof: Comparison) -> Json:
    plan, native, rust = proof.plan, proof.native, proof.rust
    initial_native, initial_rust = proof.initial_native, proof.initial_rust
    label, expected, actual = proof.label, proof.expected, proof.actual
    compare_fields(initial_native, initial_rust, set())
    projected = project(native, plan)
    compare_fields(projected, rust, set())
    lifetime = lifetimes(projected, initial_native, label)
    if (
        frozenset(records(expected)) != lifetime.live
        or frozenset(records(actual)) != lifetime.live
    ):
        raise WorldCheckError("Restore checkpoint complete vehicle IDs differ")
    admitted: set[tuple[str | int, ...]] = set()
    ledger: list[Json] = []
    for creation in lifetime.fresh:
        original = duration(expected, creation)
        if duration(actual, creation) != 0:
            raise WorldCheckError("Rust fresh Restore round_trip_time must be zero")
        path = (*common_path(creation.vehicle), "round_trip_time")
        admitted.add(path)
        ledger.append(
            {
                "vehicle": int(creation.vehicle),
                "ordinal": creation.ordinal,
                "path": list(path),
                "native_value": original,
                "rust_value": 0,
            }
        )
    compare_fields(expected, actual, admitted)
    return {"schema_version": 1, "checkpoint": label, "fresh_initialization": ledger}


class Arguments(argparse.Namespace):
    plan: Path = Path()
    native: Path = Path()
    rust: Path = Path()
    initial_native: Path = Path()
    initial_rust: Path = Path()
    expected: Path = Path()
    actual: Path = Path()
    ledger: Path = Path()
    label: str = ""


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Owned Restore fresh-lifetime saved comparison"
    )
    for name in (
        "plan",
        "native",
        "rust",
        "initial-native",
        "initial-rust",
        "expected",
        "actual",
        "ledger",
    ):
        _ = parser.add_argument(f"--{name}", type=Path, required=True)
    _ = parser.add_argument("--label", required=True)
    args = Arguments()
    _ = parser.parse_args(namespace=args)
    try:
        result = compare(
            Comparison(
                read_json(args.plan),
                read_json(args.native),
                read_json(args.rust),
                read_json(args.initial_native),
                read_json(args.initial_rust),
                args.label,
                read_json(args.expected),
                read_json(args.actual),
            )
        )
        write_json(args.ledger, result)
    except (WorldCheckError, KeyError, IndexError) as error:
        print(f"Owned Restore comparison rejected: {error}", file=sys.stderr)
        return 1
    print(f"PASS owned Restore checkpoint {args.label}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
