from __future__ import annotations

from scripts.backup_sale_bindings import edit_job, native_job, prepare_evidence
from scripts.backup_sale_evidence import membership, pair_evidence, witnesses
from scripts.backup_sale_run import BackupSaleRun, member
from scripts.context_ci_support import exact
from scripts.depot_build_archive import bounded_paths
from scripts.grf_control_evidence import sequence, text
from scripts.world_check_support import Json, WorldCheckError, at, read_json


def critical_witnesses(native: Json, requirements: Json) -> None:
    for requirement in sequence(requirements):
        path: list[str | int] = []
        for item in sequence(at(requirement, ("path",))):
            if type(item) not in (str, int):
                raise WorldCheckError("Invalid witness path")
            match item:
                case str() | int():
                    path.append(item)
                case _:
                    raise WorldCheckError("Invalid witness path")
        if not path:
            raise WorldCheckError("Empty witness path")
        try:
            actual = at(native, tuple(path))
        except (KeyError, IndexError) as error:
            raise WorldCheckError("Missing lifecycle witness field") from error
        if not exact(actual, at(requirement, ("value",))):
            raise WorldCheckError("Backup-enabled lifecycle witness differs")


def validate(job: BackupSaleRun, layout: Json) -> Json:
    if not job.backup_enabled:
        raise WorldCheckError("Enabled validator requires its distinct observer mode")
    prepare_evidence(job)
    membership(at(job.fixtures, ("cases",)), at(layout, ("cases",)))
    cases = [text(v) for v in sequence(at(layout, ("cases",)))]
    membership(
        [at(entry, ("directory",)) for entry in sequence(at(job.fixtures, ("jobs",)))],
        at(layout, ("jobs",)),
    )
    for entry in sequence(at(job.fixtures, ("jobs",))):
        match at(entry, ("kind",)):
            case "native":
                native_job(job, entry)
            case "edit":
                edit_job(job, entry)
            case _:
                raise WorldCheckError("Unknown enabled-sale fixture operation")
    actions = saves = commands = executed = successful = 0
    for name in cases:
        native = read_json(
            member(job.output / "results", name) / "original/native/results.json"
        )
        witnesses(native, at(layout, ("command_witnesses", name)))
        critical_witnesses(native, at(layout, ("critical_witnesses", name)))
        count, saved = pair_evidence(job, name)
        actions += count
        saves += saved
        for row in sequence(at(native, ("actions",))):
            if at(row, ("input", "op")) == "command":
                receipt = at(row, ("result", "receipt"))
                commands += 1
                executed += at(receipt, ("exec",)) is not None
                successful += at(receipt, ("result", "success")) is True
    coverage: Json = {
        "cases": len(cases),
        "actions": actions,
        "snapshots": len(cases) * 2 + actions * 2,
        "saves": saves,
        "command_receipts": commands,
        "executed_commands": executed,
        "successful_results": successful,
    }
    if not exact(coverage, at(layout, ("coverage",))):
        raise WorldCheckError("Backup-enabled executed coverage differs")
    _ = bounded_paths(
        job.output / "results",
        {text(v) for v in sequence(at(layout, ("result_paths",)))},
    )
    return coverage
