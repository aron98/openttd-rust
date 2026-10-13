from __future__ import annotations

import sys
from copy import deepcopy
from typing import Final

from scripts.backup_sale_guards import observer_controls, validate_observer_controls
from scripts.backup_sale_run import SELECTOR, BackupSaleRun
from scripts.context_ci_support import exact
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
    ("cost", ("actions", 5, "result", "receipt", "result", "cost")),
    (
        "copy-wait",
        ("actions", 5, "after", "orders", "backups", 0, "orders", 0, "wait_time"),
    ),
    ("physical-slot", ("actions", 5, "after", "orders", "backups", 0, "pool_slot")),
    ("surviving-user", ("actions", 7, "after", "orders", "backups", 0, "user")),
    (
        "group-engines",
        ("actions", 5, "after", "sale", "groups", "0", "all", "engines", "116"),
    ),
)
UNITS: Final = (
    "runtime::order_state::backup_sale_tests::renewal_guard_post_empty_backups",
    "runtime::order_state::backup_sale_tests::renewal_guard_estimate_empty_backups",
    "runtime::order_state::backup_sale_tests::renewal_guard_post_live_backups",
    "runtime::order_state::backup_sale_tests::renewal_guard_estimate_live_backups",
    "runtime::order_state::backup_sale_tests::failed_sale_candidate_rolls_back_backup_retarget_and_all_allocators",
    "runtime::order_state::backup_sale_tests::composed_sale_preflights_native_slot_index_deletion_boundary",
    "runtime::order_state::backup_sale_tests::live_backup_host_boundary_preserves_native_gates_and_empty_backup_behavior",
    "runtime::order_state::backup_sale_tests::copied_backups_survive_sale_and_sp_projection_without_mutation",
)


def controls(job: BackupSaleRun) -> None:
    output = job.output / "controls"
    output.mkdir()
    case = job.output / "results/matrix/copy-sale-clear"
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
        [runner, "--exact", "runtime::backup_sale_native::absent", "--ignored"],
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
            "OTTD_BACKUP_SALE_CASE=matrix/estimate",
            sys.executable,
            str(job.root / "scripts/check-backup-sale.py"),
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
    observer_controls(job)
    native_admission(
        OrderRun(job.root, job.output, job.oracle, job.binaries, job.fixtures)
    )


def validate_semantic(job: BackupSaleRun) -> None:
    output = job.output / "controls"
    case = job.output / "results/matrix/copy-sale-clear"
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


def validate_controls(job: BackupSaleRun) -> None:
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
    validate_observer_controls(job)
    validate_native_guard(job.root, job.output, job.oracle)


def validate_negative(job: BackupSaleRun) -> None:
    output = job.output / "controls"
    expected_zero: Json = [
        job.executable("runner"),
        "--exact",
        "runtime::backup_sale_native::absent",
        "--ignored",
    ]
    expected_subset: Json = [
        "env",
        "OTTD_BACKUP_SALE_CASE=matrix/estimate",
        sys.executable,
        str(job.root / "scripts/check-backup-sale.py"),
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
