# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through scripts/check-occupancy-ci.py.
from __future__ import annotations

from pathlib import Path

from scripts.context_ci_support import OCCUPANCY_TEST, exact, process, verify_identity
from scripts.depot_build_archive import bounded_paths
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import sequence, text
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


def compare_occupancy(native: Json, rust: Json, canonical: Json, reload: Json) -> None:
    witness = at(native, ("occupancy",))
    if not exact(at(witness, ("before",)), at(witness, ("after",))):
        raise WorldCheckError("Occupancy live restoration differs")
    for value in (native, canonical, reload):
        if not exact(
            {
                "depot": at(value, ("runtime",)),
                "vehicles": at(value, ("vehicles",)),
            },
            at(witness, ("before",)),
        ):
            raise WorldCheckError("Independent occupancy reload/live snapshot differs")
    if not exact(
        at(witness, ("hash_restored",)),
        {
            "current": True,
            "previous": True,
            "next": True,
        },
    ) or not exact(at(witness, ("members_before",)), at(witness, ("members_after",))):
        raise WorldCheckError("Native vehicle hash membership/order was not restored")
    compare_vectors(witness, rust)


def compare_vectors(witness: Json, rust: Json) -> None:
    vectors = sequence(at(witness, ("vectors",)))
    compared = sequence(rust)
    if len(vectors) != 2 or len(compared) != 2:
        raise WorldCheckError("Incomplete occupancy vectors")
    if not exact(at(witness, ("members_before",)), [at(vectors[0], ("vehicle",))]):
        raise WorldCheckError("Occupancy membership does not select actual vehicle")
    for field in ("vehicle", "tile", "maximum_z"):
        if not exact(at(vectors[0], (field,)), at(vectors[1], (field,))):
            raise WorldCheckError("Occupancy vectors use different objects or cutoffs")
    for index, (row, actual) in enumerate(zip(vectors, compared, strict=True)):
        maximum = at(row, ("maximum_z",))
        match maximum:
            case int() if not isinstance(maximum, bool):
                if at(row, ("z",)) != maximum + index:
                    raise WorldCheckError("Wrong occupancy cutoff coordinate")
            case _:
                raise WorldCheckError("Invalid occupancy coordinate")
        if not exact(
            actual,
            {
                key: at(row, (key,))
                for key in ("success", "error_id", "cost", "expenses")
            },
        ) or at(actual, ("success",)) is not bool(index):
            raise WorldCheckError("Pure occupancy result differs")


def validate_occupancy(root: Path, output: Path, layout: Json) -> None:
    expected = {text(value) for value in sequence(at(layout, ("paths",)))}
    original = read_json(output / "manifest.json")
    match original:
        case dict() if set(original) == expected:
            for name, sha in original.items():
                if digest(output / name) != sha:
                    raise WorldCheckError("Original occupancy artifact changed")
        case _:
            raise WorldCheckError("Occupancy path set differs")
    _ = bounded_paths(
        output,
        expected
        | {
            "manifest.json",
            "summary.json",
            "ci-invocation/stdout.log",
            "ci-invocation/stderr.log",
            "ci-invocation/argv.json",
            "ci-invocation/environment.json",
            "ci-invocation/process.json",
        },
    )
    verify_identity(root, output, occupancy=True)
    identities = read_json(output / "test-binaries.json")
    binary = Path(text(at(identities, ("ottd_sim", "retained"))))
    for name in ("flat", "sloped"):
        case = output / name
        process(case / "logs" / OCCUPANCY_TEST, binary, OCCUPANCY_TEST)
        compare_occupancy(
            read_json(case / "vectors/depot-runtime.json"),
            read_json(case / "rust-vectors.json"),
            read_json(case / "canonical/depot-runtime.json"),
            read_json(case / "reload/depot-runtime.json"),
        )
    for name in ("prepare_fleet_source", "prepare_depot_inputs"):
        process(
            output / "logs" / name,
            Path(text(at(identities, ("native_depot_build", "retained")))),
            name,
        )
    rejection = output / "rejection"
    if (
        at(read_json(rejection / "process.json"), ("returncode",)) != 101
        or "pure occupancy result differs from original"
        not in (rejection / "stdout.log").read_text()
        or (output / "corruption/rust-vectors.json").exists()
    ):
        raise WorldCheckError("Actual occupancy mutation was not rejected")
    argv = sequence(read_json(rejection / "argv.json"))
    if str(binary) not in argv or OCCUPANCY_TEST not in argv or "--exact" not in argv:
        raise WorldCheckError("Occupancy rejection used wrong executable")
    write_json(
        output / "coverage.json",
        {
            "sources": ["flat", "sloped"],
            "vectors": 4,
            "mutations_rejected": 1,
            "pure_test_invocations": 2,
            "input_preparations": 2,
            "complete_restoration_assertions": True,
        },
    )
