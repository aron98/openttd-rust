from __future__ import annotations

from scripts.backup_sale_bindings import edit_job, prepare_evidence
from scripts.backup_sale_run import BackupSaleRun, member
from scripts.context_ci_support import exact
from scripts.grf_control_evidence import sequence, text
from scripts.owned_restore_capacity import full_lists
from scripts.owned_restore_native_evidence import native_receipt
from scripts.owned_restore_run import RestoreRun
from scripts.world_check_support import WorldCheckError, at, read_json


def validate_preparation(job: RestoreRun) -> None:
    prepared = BackupSaleRun(
        job.root, job.output, job.oracle, job.binaries, job.fixtures
    )
    prepare_evidence(prepared)
    results = job.output / "results"
    for entry in sequence(at(job.fixtures, ("jobs",))):
        match at(entry, ("kind",)):
            case "edit":
                edit_job(prepared, entry)
            case "native":
                directory = member(results, text(at(entry, ("directory",))))
                source = member(results, text(at(entry, ("source",))))
                if not exact(
                    read_json(directory / "actions.json"), at(entry, ("descriptor",))
                ):
                    raise WorldCheckError("Restore prepared descriptor differs")
                native_receipt(job, directory, source)
            case _:
                raise WorldCheckError("Unknown Restore preparation")
    if (
        full_lists((results / "matrix/copies.sav").read_bytes())
        != (results / "preparation/capacity.sav").read_bytes()
    ):
        raise WorldCheckError("Restore complete capacity saved fixture differs")
