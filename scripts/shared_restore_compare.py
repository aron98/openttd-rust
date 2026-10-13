from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path

from scripts.shared_restore_strict import compare_live, compare_saved, creations
from scripts.world_check_support import Json, WorldCheckError, read_json, write_json


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
    if proof.label != "after":
        raise WorldCheckError("Shared Restore checkpoint differs")
    compare_saved(proof.initial_native, proof.initial_rust, ())
    compare_live(proof.native, proof.rust, proof.plan)
    created = creations(proof.native)
    compare_saved(proof.expected, proof.actual, created)
    return {"created": list(created), "duration_exceptions": 0}


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


def main() -> None:
    parser = argparse.ArgumentParser(description="Strict shared Restore comparison")
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
        _ = parser.add_argument("--" + name, type=Path, required=True)
    _ = parser.add_argument("--label", required=True)
    args = Arguments()
    _ = parser.parse_args(namespace=args)
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


if __name__ == "__main__":
    main()
