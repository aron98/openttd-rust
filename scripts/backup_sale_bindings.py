from __future__ import annotations

from pathlib import Path

from scripts.backup_sale_run import CONFIG, BackupSaleRun, member
from scripts.context_ci_support import exact
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import text
from scripts.order_state_evidence import command
from scripts.world_check_support import Json, WorldCheckError, at, read_json


def native_job(job: BackupSaleRun, entry: Json) -> None:
    results = job.output / "results"
    case = member(results, text(at(entry, ("directory",))))
    source = member(results, text(at(entry, ("source",))))
    native = case / "original"
    descriptor = at(entry, ("descriptor",))
    if (native / "openttd.cfg").read_text() != CONFIG:
        raise WorldCheckError("Pinned single-player config differs")
    if not exact(read_json(case / "actions.json"), descriptor):
        raise WorldCheckError("Pinned depot descriptor changed")
    before = read_json(native / "bindings-before.json")
    if not exact(before, read_json(native / "bindings-after.json")):
        raise WorldCheckError("Depot native before/after identity differs")
    expected: Json = {
        str(path): digest(path)
        for path in (job.oracle, source, case / "actions.json", native / "openttd.cfg")
    }
    if not exact(before, expected):
        raise WorldCheckError("Depot native source/executable/config identity differs")
    if not exact(
        read_json(native / "process.json"),
        {"returncode": 0, "expected": 0, "timeout_seconds": 90},
    ):
        raise WorldCheckError("Original depot command process failed")
    argv = [
        str(job.oracle),
        "-x",
        "-c",
        str(native / "openttd.cfg"),
        "-snull",
        "-mnull",
        "-d",
        "sl=2",
        "-vnull:ticks=100000000",
        "-g",
        str(source),
    ]
    if not exact(read_json(native / "argv.json"), list(argv)):
        raise WorldCheckError("Original depot process arguments changed")
    variables: Json = {
        job.observer_mode: "1",
        "OTTD_ORDER_STATE_PATH": str(native / "loaded.json"),
        "OTTD_WORLD_PATH": str(native / "saved-world.json"),
        "OTTD_WORLD_SCHEMA_PATH": str(native / "saved-schema.json"),
        "OTTD_ORDER_FIXTURE_PATH": str(case / "actions.json"),
        "OTTD_ORDER_FIXTURE_DIR": str(native / "native"),
    }
    if not exact(read_json(native / "environment.json"), variables):
        raise WorldCheckError("Depot observer environment changed")
    if not exact(
        read_json(case / "inputs.json"),
        {
            "source": str(source),
            "source_sha256": digest(source),
            "descriptor_sha256": digest(case / "actions.json"),
            "oracle_sha256": digest(job.oracle),
        },
    ):
        raise WorldCheckError("Depot input binding differs")


def edit_job(job: BackupSaleRun, entry: Json) -> None:
    results = job.output / "results"
    directory = member(results, text(at(entry, ("directory",))))
    source = member(results, text(at(entry, ("source",))))
    destination = member(results, text(at(entry, ("output",))))
    descriptor = destination.with_suffix(".edit.json")
    if not exact(read_json(descriptor), at(entry, ("edits",))):
        raise WorldCheckError("Pinned depot fixture edits differ")
    command(
        directory,
        [
            job.executable("cli"),
            "edit-world",
            str(source),
            str(descriptor),
            str(destination),
            "--compression",
            "none",
        ],
    )
    if not exact(
        read_json(directory / "bindings.json"),
        {
            "source": str(source),
            "source_before": digest(source),
            "source_after": digest(source),
            "output": str(destination),
            "output_sha256": digest(destination),
            "descriptor_sha256": digest(descriptor),
            "cli_sha256": digest(Path(job.executable("cli"))),
        },
    ):
        raise WorldCheckError("Depot generated input binding changed")


def prepare_evidence(job: BackupSaleRun) -> None:
    results = job.output / "results"
    command(
        results / "prepare-command",
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
    )
    expected = (
        f"ORACLE={job.oracle}\nORACLE_SHA256={digest(job.oracle)}\n"
        f"INPUT={job.root}/fixtures/replay/clear-v362.sav\n"
        f"INPUT_SHA256={digest(job.root / 'fixtures/replay/clear-v362.sav')}\n"
        f"CONFIG_SHA256={digest(job.root / 'scripts/reference.cfg')}\n"
        "UPSTREAM=14ec60f248547d4d062a1160f0fc26d742319888\n"
        "PHASE=depot-pool-company-road-infrastructure\n"
    )
    if (results / "matrix/prepared/invocation.txt").read_text() != expected:
        raise WorldCheckError("Original prepared input provenance differs")
