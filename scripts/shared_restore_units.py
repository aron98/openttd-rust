from __future__ import annotations

from typing import Final

from scripts.gameplay_foundations import require_test
from scripts.grf_control_run import ControlRun
from scripts.order_state_evidence import command
from scripts.owned_restore_run import RestoreRun

SELECTORS: Final = (
    "runtime::order_state::restore_tests::shared_restore_rejoins_existing_list_without_allocating_orders",
    "runtime::order_state::shared_restore_tests::shared_restore_copies_properties_uses_live_orders_and_consumes_once",
    "runtime::order_state::shared_restore_tests::shared_restore_estimate_and_failure_preserve_all_state",
    "runtime::order_state::shared_restore_tests::shared_restore_rejects_unproved_domains_without_publication",
    "runtime::order_state::shared_restore_tests::shared_restore_candidate_failure_rolls_back_links_and_consumption",
    "runtime::order_state::restore_boundary_tests::restore_rejects_matching_clone_group_and_live_non_sp_before_mutation",
)


def units(job: RestoreRun) -> None:
    control = ControlRun(job.root, job.output, job.oracle)
    for index, selector in enumerate(SELECTORS):
        arguments = [job.executable("runner"), "--exact", selector, "--nocapture"]
        result = control.run(f"shared-unit-{index}", arguments)
        require_test(result.stdout, selector)
    validate_units(job)


def validate_units(job: RestoreRun) -> None:
    for index, selector in enumerate(SELECTORS):
        directory = job.output / "logs" / f"shared-unit-{index}"
        arguments = [job.executable("runner"), "--exact", selector, "--nocapture"]
        command(directory, arguments)
        require_test((directory / "stdout.log").read_text(), selector)
