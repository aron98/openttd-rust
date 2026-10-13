from __future__ import annotations

from scripts.backup_sale_run import member
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import sequence, text
from scripts.owned_restore_run import RestoreRun
from scripts.shared_restore_capacity import prepare_capacity
from scripts.world_check_support import WorldCheckError, at, run, write_json


def prepare(job: RestoreRun) -> None:
    results = job.output / "results"
    _ = run(
        [
            "cmake",
            f"-DORACLE={job.oracle}",
            f"-DRUN_DIR={results}/matrix/prepared",
            f"-DINPUT={job.root}/fixtures/replay/clear-v362.sav",
            f"-DCONFIG={job.root}/scripts/reference.cfg",
            "-DPREPARE=ON",
            "-DVECTORS=OFF",
            "-P",
            str(job.root / "scripts/run-depot-runtime-reference.cmake"),
        ],
        results / "prepare-command",
    )
    for entry in sequence(at(job.fixtures, ("jobs",))):
        directory = member(results, text(at(entry, ("directory",))))
        source = member(results, text(at(entry, ("source",))))
        match at(entry, ("kind",)):
            case "native":
                job.native(source, directory, at(entry, ("descriptor",)))
            case "edit":
                destination = member(results, text(at(entry, ("output",))))
                descriptor = destination.with_suffix(".edit.json")
                descriptor.parent.mkdir(parents=True, exist_ok=True)
                write_json(descriptor, at(entry, ("edits",)))
                before = digest(source)
                _ = run(
                    [
                        job.executable("cli"),
                        "edit-world",
                        str(source),
                        str(descriptor),
                        str(destination),
                        "--compression",
                        "none",
                    ],
                    directory,
                )
                if digest(source) != before:
                    raise WorldCheckError("Restore preparation changed source")
                write_json(
                    directory / "bindings.json",
                    {
                        "source": str(source),
                        "source_before": before,
                        "source_after": digest(source),
                        "output": str(destination),
                        "output_sha256": digest(destination),
                        "descriptor_sha256": digest(descriptor),
                        "cli_sha256": digest(member(job.output, "bin/cli")),
                    },
                )
            case _:
                raise WorldCheckError("Unknown Restore fixture operation")
    prepare_capacity(job)
