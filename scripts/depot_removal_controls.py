from __future__ import annotations

import sys
from copy import deepcopy
from typing import Final

from scripts.context_ci_support import exact
from scripts.depot_removal_run import SELECTOR, RemovalRun
from scripts.gameplay_foundations import require_test
from scripts.order_state_controls import native_admission
from scripts.order_state_fixtures import OrderRun
from scripts.order_state_provenance import validate_native_guard
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    read_json,
    replace,
    run,
    write_json,
)

MUTATIONS: Final[tuple[tuple[str, tuple[str | int, ...]], ...]] = (
    ("cost", ("actions", 2, "result", "receipt", "result", "cost")),
    ("map", ("actions", 2, "after", "tiles", 1556, 4)),
    ("shared-duration", ("initial", "orders", "lists", 0, "total_duration")),
    ("backup-slot", ("actions", 0, "after", "orders", "backups", 0, "pool_slot")),
    ("infrastructure", ("actions", 2, "after", "depot", "road", "0", 0)),
)
UNITS: Final = (
    "runtime::depot_removal::tests::occupied_depot_keeps_live_same_tile_backups_and_construction_expense",
    "runtime::depot_removal::tests::transaction_rejects_loaded_native_backup_index_before_any_publication",
)


def controls(job: RemovalRun) -> None:
    output = job.output / "controls"
    output.mkdir()
    case = job.output / "results/matrix/shared-remove-reuse"
    original = read_json(case / "rust/results.json")
    for name, path in MUTATIONS:
        value = deepcopy(original)
        current = at(value, path)
        if type(current) is not int:
            raise WorldCheckError("Mutation target is not an integer")
        replace(value, path, current + 1)
        changed = output / f"{name}.json"
        write_json(changed, value)
        result = run(
            [
                job.executable("cli"),
                "compare",
                str(case / "native-state.json"),
                str(changed),
            ],
            output / name,
            expected=1,
        )
        if not result.stderr.startswith("Error: $"):
            raise WorldCheckError("Semantic mutation failed for unrelated reason")
    runner = job.executable("runner")
    zero = run(
        [runner, "--exact", "runtime::depot_removal_native::absent", "--ignored"],
        output / "zero",
    )
    try:
        require_test(zero.stdout, SELECTOR)
    except WorldCheckError as error:
        write_json(
            output / "zero-rejected.json", {"rejected": True, "reason": str(error)}
        )
    else:
        raise WorldCheckError("Actual zero tests were accepted")
    result = run(
        [
            "env",
            "OTTD_DEPOT_REMOVAL_CASE=matrix/estimate",
            sys.executable,
            str(job.root / "scripts/check-depot-removal.py"),
        ],
        output / "subset",
        expected=1,
    )
    if "refuses subset or inherited case environment" not in result.stderr:
        raise WorldCheckError("Actual subset dispatcher admission was not rejected")
    for index, selector in enumerate(UNITS):
        result = run(
            [runner, "--exact", selector, "--nocapture"], output / f"unit-{index}"
        )
        require_test(result.stdout, selector)
    native_admission(
        OrderRun(job.root, job.output, job.oracle, job.binaries, job.fixtures)
    )


def validate_semantic(job: RemovalRun) -> None:
    output = job.output / "controls"
    case = job.output / "results/matrix/shared-remove-reuse"
    original = read_json(case / "rust/results.json")
    for name, path in MUTATIONS:
        value = deepcopy(original)
        current = at(value, path)
        if type(current) is not int:
            raise WorldCheckError("Invalid mutation target")
        replace(value, path, current + 1)
        if not exact(value, read_json(output / f"{name}.json")) or not exact(
            read_json(output / name / "process.json"), {"returncode": 1, "expected": 1}
        ):
            raise WorldCheckError("Depot semantic mutation/rejection differs")
        expected: Json = [
            job.executable("cli"),
            "compare",
            str(case / "native-state.json"),
            str(output / f"{name}.json"),
        ]
        if not exact(read_json(output / name / "argv.json"), expected) or not (
            output / name / "stderr.log"
        ).read_text().startswith("Error: $"):
            raise WorldCheckError("Actual semantic mutation command missing")


def validate_controls(job: RemovalRun) -> None:
    validate_semantic(job)
    output = job.output / "controls"
    validate_negative(job)
    for index, selector in enumerate(UNITS):
        require_test((output / f"unit-{index}/stdout.log").read_text(), selector)
        if not exact(
            read_json(output / f"unit-{index}/argv.json"),
            [job.executable("runner"), "--exact", selector, "--nocapture"],
        ) or not exact(
            read_json(output / f"unit-{index}/process.json"),
            {"returncode": 0, "expected": 0},
        ):
            raise WorldCheckError("Actual rollback unit command differs")
    validate_native_guard(job.root, job.output, job.oracle)


def validate_negative(job: RemovalRun) -> None:
    output = job.output / "controls"
    expected_zero: Json = [
        job.executable("runner"),
        "--exact",
        "runtime::depot_removal_native::absent",
        "--ignored",
    ]
    expected_subset: Json = [
        "env",
        "OTTD_DEPOT_REMOVAL_CASE=matrix/estimate",
        sys.executable,
        str(job.root / "scripts/check-depot-removal.py"),
    ]
    if not exact(read_json(output / "zero/argv.json"), expected_zero) or not exact(
        read_json(output / "subset/argv.json"), expected_subset
    ):
        raise WorldCheckError("Zero/subset executable arguments differ")
    zero = (output / "zero/stdout.log").read_text()
    if "0 passed; 0 failed; 0 ignored" not in zero or not exact(
        read_json(output / "zero/process.json"), {"returncode": 0, "expected": 0}
    ):
        raise WorldCheckError("Actual zero-test control missing")
    try:
        require_test(zero, SELECTOR)
    except WorldCheckError as error:
        if not exact(
            read_json(output / "zero-rejected.json"),
            {"rejected": True, "reason": str(error)},
        ):
            raise WorldCheckError(
                "Actual zero-test rejection receipt differs"
            ) from error
    else:
        raise WorldCheckError("Zero-test admission succeeded")
    if (
        not exact(
            read_json(output / "subset/process.json"), {"returncode": 1, "expected": 1}
        )
        or "refuses subset or inherited case environment"
        not in (output / "subset/stderr.log").read_text()
    ):
        raise WorldCheckError("Subset admission control missing")
