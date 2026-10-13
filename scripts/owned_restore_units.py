from __future__ import annotations

from typing import Final

from scripts.gameplay_foundations import require_test
from scripts.grf_control_run import ControlRun
from scripts.order_state_evidence import command
from scripts.owned_restore_run import RestoreRun

SELECTORS: Final = (
    "runtime::order_state::restore_tests::purchase_restores_owned_copy_and_consumes_backup_in_one_lifetime",
    "runtime::order_state::restore_tests::purchase_with_unrelated_live_user_preserves_backup_and_builds_empty_orders",
    "runtime::order_state::restore_tests::matching_restore_estimate_does_not_consume_backup_or_allocate_lists",
    "runtime::order_state::restore_tests::restore_post_zero_matches_server_user_and_preserves_primitive_zero",
    "runtime::order_state::restore_tests::restore_empty_owned_backup_consumes_row_without_allocating_order_list",
    "runtime::order_state::restore_boundary_tests::restore_poor_and_invalid_engine_leave_backups_rng_and_allocators_unchanged",
    "runtime::order_state::restore_boundary_tests::restore_rejects_matching_clone_group_and_live_non_sp_before_mutation",
    "runtime::order_state::restore_boundary_tests::restore_preserves_nonempty_renewal_rows_and_company_link",
    "runtime::order_state::restore_boundary_tests::restore_cache_failure_rolls_back_consumption_list_allocation_money_and_rng",
    "runtime::order_state::restore_boundary_tests::restore_copies_native_bits_and_skips_implicit_orders_without_copying_current_order",
    "runtime::order_state::restore_boundary_tests::restore_full_list_pool_consumes_backup_and_properties_without_allocating",
)


def units(job: RestoreRun) -> None:
    control = ControlRun(job.root, job.output, job.oracle)
    for index, selector in enumerate(SELECTORS):
        arguments = [job.executable("runner"), "--exact", selector, "--nocapture"]
        result = control.run(f"restore-unit-{index}", arguments)
        require_test(result.stdout, selector)
    validate_units(job)


def validate_units(job: RestoreRun) -> None:
    for index, selector in enumerate(SELECTORS):
        directory = job.output / "logs" / f"restore-unit-{index}"
        arguments = [job.executable("runner"), "--exact", selector, "--nocapture"]
        command(directory, arguments)
        require_test((directory / "stdout.log").read_text(), selector)
