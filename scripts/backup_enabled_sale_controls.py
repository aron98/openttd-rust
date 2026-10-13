from __future__ import annotations

import sys
from copy import deepcopy
from dataclasses import replace as replace_job
from typing import Final

from scripts.backup_enabled_sale_guards import (
    observer_controls,
    validate_observer_controls,
)
from scripts.backup_enabled_sale_provenance import verify_all
from scripts.backup_sale_controls import MUTATIONS, validate_semantic
from scripts.backup_sale_run import BackupSaleRun
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

UNITS: Final = (
    *(
        "runtime::order_state::backup_enabled_tests::" + name
        for name in (
            "backup_enabled_sale_creates_owned_copy_before_last_list_free",
            "backup_enabled_post_zero_maps_to_server_but_primitive_zero_stays_literal",
            "backup_enabled_estimate_keeps_existing_user_backup_and_every_allocator",
            "backup_enabled_same_slot_replacement_survives_candidate_clear_vehicle",
            "backup_enabled_full_pool_checks_capacity_before_clear_vehicle_frees_clones",
            "backup_enabled_full_pool_replaces_user_then_preserves_reused_copy_slot",
            "backup_enabled_failed_candidate_rolls_back_replacement_detach_and_refund",
            "backup_enabled_poor_and_invalid_owner_fail_before_replacing_user_backup",
            "backup_enabled_non_sp_rejects_even_when_backup_pool_empty",
            "backup_enabled_preserves_renewal_guard_for_post_estimate_empty_live_backups",
        )
    ),
    "runtime::order_state::backup_sale_tests::composed_sale_preflights_native_slot_index_deletion_boundary",
)


def zero_argv(job: BackupSaleRun) -> list[str]:
    return [
        job.executable("runner"),
        "--exact",
        "runtime::backup_enabled_sale_native::absent",
        "--ignored",
    ]


def subset_argv(job: BackupSaleRun) -> list[str]:
    return [
        "env",
        "OTTD_BACKUP_ENABLED_SALE_CASE=matrix/estimate",
        sys.executable,
        str(job.root / "scripts/check-backup-enabled-sale.py"),
    ]


def wrong_executable(job: BackupSaleRun, layout: Json) -> tuple[Json, str]:
    binaries = deepcopy(job.binaries)
    replace(binaries, ("runner",), deepcopy(at(binaries, ("cli",))))
    try:
        verify_all(replace_job(job, binaries=binaries), layout)
    except WorldCheckError as error:
        if "actual Cargo selection" not in str(error):
            raise WorldCheckError(
                "Wrong executable failed for unrelated reason"
            ) from error
        return binaries, str(error)
    raise WorldCheckError("Self-consistent wrong executable was accepted")


def controls(job: BackupSaleRun, layout: Json) -> None:
    output = job.output / "controls"
    output.mkdir()
    case = job.output / "results/matrix/copy-sale-clear"
    original = read_json(case / "rust/results.json")
    for name, path in MUTATIONS:
        value = deepcopy(original)
        current = at(value, path)
        if type(current) is not int:
            raise WorldCheckError("Mutation target is not integer")
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
            raise WorldCheckError("Mutation failed for unrelated reason")
    zero = run(zero_argv(job), output / "zero")
    try:
        require_test(zero.stdout, job.selector)
    except WorldCheckError as error:
        write_json(
            output / "zero-rejected.json", {"rejected": True, "reason": str(error)}
        )
    else:
        raise WorldCheckError("Actual zero tests accepted")
    _ = run(subset_argv(job), output / "subset", expected=1)
    for index, selector in enumerate(UNITS):
        result = run(
            [job.executable("runner"), "--exact", selector, "--nocapture"],
            output / f"unit-{index}",
        )
        require_test(result.stdout, selector)
    observer_controls(job)
    native_admission(
        OrderRun(job.root, job.output, job.oracle, job.binaries, job.fixtures)
    )
    binaries, reason = wrong_executable(job, layout)
    write_json(
        output / "wrong-executable.json",
        {"binaries": binaries, "rejected": True, "reason": reason},
    )


def validate_controls(job: BackupSaleRun, layout: Json) -> None:
    output = job.output / "controls"
    validate_semantic(job)
    for name, argv, expected in (
        ("zero", zero_argv(job), 0),
        ("subset", subset_argv(job), 1),
    ):
        if not exact(read_json(output / name / "argv.json"), list(argv)) or not exact(
            read_json(output / name / "process.json"),
            {"returncode": expected, "expected": expected},
        ):
            raise WorldCheckError("Zero/subset invocation differs")
    zero = (output / "zero/stdout.log").read_text()
    if "0 passed; 0 failed; 0 ignored" not in zero:
        raise WorldCheckError("Real zero execution missing")
    try:
        require_test(zero, job.selector)
    except WorldCheckError as error:
        if not exact(
            read_json(output / "zero-rejected.json"),
            {"rejected": True, "reason": str(error)},
        ):
            raise WorldCheckError("Zero rejection receipt differs") from error
    else:
        raise WorldCheckError("Zero admitted")
    if (
        "refuses subset or inherited case environment"
        not in (output / "subset/stderr.log").read_text()
    ):
        raise WorldCheckError("Subset refusal missing")
    validate_units(job)
    validate_observer_controls(job)
    validate_native_guard(job.root, job.output, job.oracle)
    binaries, reason = wrong_executable(job, layout)
    if not exact(
        read_json(output / "wrong-executable.json"),
        {"binaries": binaries, "rejected": True, "reason": reason},
    ):
        raise WorldCheckError("Actual executable admission receipt differs")


def validate_units(job: BackupSaleRun) -> None:
    output = job.output / "controls"
    for index, selector in enumerate(UNITS):
        directory = output / f"unit-{index}"
        require_test((directory / "stdout.log").read_text(), selector)
        if not exact(
            read_json(directory / "argv.json"),
            [job.executable("runner"), "--exact", selector, "--nocapture"],
        ) or not exact(
            read_json(directory / "process.json"), {"returncode": 0, "expected": 0}
        ):
            raise WorldCheckError("Focused actual unit differs")
