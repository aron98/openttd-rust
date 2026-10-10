from __future__ import annotations

import sys
from copy import deepcopy
from typing import Final

from scripts.context_ci_support import exact
from scripts.gameplay_foundations import require_test
from scripts.order_state_controls import native_admission
from scripts.order_state_fixtures import OrderRun
from scripts.order_state_provenance import validate_native_guard
from scripts.ordered_sale_run import SELECTOR, OrderedSaleRun
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
    ("cost", ("actions", 0, "result", "receipt", "result", "cost")),
    ("shared-head", ("actions", 0, "after", "orders", "lists", 0, "first_shared")),
    ("shared-duration", ("initial", "orders", "lists", 0, "total_duration")),
    ("pool", ("actions", 0, "after", "orders", "list_pool", "items")),
    (
        "group-engines",
        ("actions", 0, "after", "sale", "groups", "0", "all", "engines", "116"),
    ),
)
UNITS: Final = (
    "runtime::order_state::sale_tests::ordered_estimate_and_failed_prepare_leave_all_order_lifetimes_unchanged",
    "runtime::order_state::sale_tests::sole_order_pool_free_reuses_lowest_identity_without_clean_pool",
    "runtime::order_state::sale_tests::ordered_sale_loading_and_live_backup_stay_explicit_boundaries",
)


def controls(job: OrderedSaleRun) -> None:
    output = job.output / "controls"
    output.mkdir()
    case = job.output / "results/matrix/head"
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
        [runner, "--exact", "runtime::ordered_sale_native::absent", "--ignored"],
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
            "OTTD_ORDERED_SALE_CASE=matrix/estimate",
            sys.executable,
            str(job.root / "scripts/check-ordered-sale.py"),
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
    mixed_mode(job)
    native_admission(
        OrderRun(job.root, job.output, job.oracle, job.binaries, job.fixtures)
    )


def validate_semantic(job: OrderedSaleRun) -> None:
    output = job.output / "controls"
    case = job.output / "results/matrix/head"
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


def validate_controls(job: OrderedSaleRun) -> None:
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
    validate_mixed_mode(job)
    validate_native_guard(job.root, job.output, job.oracle)


def validate_negative(job: OrderedSaleRun) -> None:
    output = job.output / "controls"
    expected_zero: Json = [
        job.executable("runner"),
        "--exact",
        "runtime::ordered_sale_native::absent",
        "--ignored",
    ]
    expected_subset: Json = [
        "env",
        "OTTD_ORDERED_SALE_CASE=matrix/estimate",
        sys.executable,
        str(job.root / "scripts/check-ordered-sale.py"),
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


def mixed_argv(job: OrderedSaleRun) -> list[str]:
    case = job.output / "results/matrix/head"
    directory = job.output / "controls/mixed-mode"
    return [
        "env",
        "OTTD_ORDERED_SALE_OBSERVE=1",
        "OTTD_DEPOT_REMOVAL_OBSERVE=1",
        f"OTTD_ORDER_STATE_PATH={directory}/loaded.json",
        f"OTTD_ORDER_FIXTURE_PATH={case}/actions.json",
        f"OTTD_ORDER_FIXTURE_DIR={directory}/forbidden",
        str(job.oracle),
        "-x",
        "-c",
        str(case / "original/openttd.cfg"),
        "-snull",
        "-mnull",
        "-vnull:ticks=100000000",
        "-g",
        str(job.output / "results/matrix/timed.sav"),
    ]


def mixed_mode(job: OrderedSaleRun) -> None:
    directory = job.output / "controls/mixed-mode"
    result = run(mixed_argv(job), directory, expected=1)
    if "lifecycle observer modes are mutually exclusive" not in result.stderr:
        raise WorldCheckError("Native mixed observer modes were not refused")
    validate_mixed_mode(job)


def validate_mixed_mode(job: OrderedSaleRun) -> None:
    directory = job.output / "controls/mixed-mode"
    if (
        not exact(read_json(directory / "argv.json"), list(mixed_argv(job)))
        or not exact(
            read_json(directory / "process.json"), {"returncode": 1, "expected": 1}
        )
        or "lifecycle observer modes are mutually exclusive"
        not in (directory / "stderr.log").read_text()
    ):
        raise WorldCheckError("Missing actual mixed observer refusal")
    if (directory / "forbidden/results.json").exists() or list(
        directory.rglob("*.sav")
    ):
        raise WorldCheckError("Mixed observer modes published gameplay output")
