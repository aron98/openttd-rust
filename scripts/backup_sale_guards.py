from __future__ import annotations

from typing import Final

from scripts.backup_sale_run import BackupSaleRun
from scripts.context_ci_support import exact
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    read_json,
    run,
    write_json,
)

MODES: Final = (
    "OTTD_BACKUP_SALE_OBSERVE",
    "OTTD_ORDERED_SALE_OBSERVE",
    "OTTD_DEPOT_REMOVAL_OBSERVE",
)
COMBINATIONS: Final = ((0, 1), (0, 2), (1, 2), (0, 1, 2))


def forbidden_descriptor() -> Json:
    return {
        "schema_version": 1,
        "case": "backup-enabled-refusal",
        "role": "sp",
        "exit": True,
        "actions": [
            {
                "op": "command",
                "request": {
                    "company": 0,
                    "mode": "post",
                    "command": {
                        "kind": "sell_vehicle",
                        "location": 1560,
                        "vehicle": 0,
                        "sell_chain": False,
                        "backup_order": True,
                        "client_id": 77,
                    },
                },
            }
        ],
    }


def argv(job: BackupSaleRun, index: int) -> list[str]:
    directory = job.output / f"controls/observer-{index}"
    variables = (
        [f"{MODES[i]}=1" for i in COMBINATIONS[index]]
        if index < len(COMBINATIONS)
        else [f"{MODES[0]}=1"]
    )
    descriptor = (
        job.output / "results/matrix/copy-sale-clear/actions.json"
        if index < len(COMBINATIONS)
        else directory / "actions.json"
    )
    return [
        "env",
        *variables,
        f"OTTD_ORDER_STATE_PATH={directory}/loaded.json",
        f"OTTD_ORDER_FIXTURE_PATH={descriptor}",
        f"OTTD_ORDER_FIXTURE_DIR={directory}/forbidden",
        str(job.oracle),
        "-x",
        "-c",
        str(job.output / "results/matrix/copy-sale-clear/original/openttd.cfg"),
        "-snull",
        "-mnull",
        "-vnull:ticks=100000000",
        "-g",
        str(job.output / "results/matrix/copies.sav"),
    ]


def observer_controls(job: BackupSaleRun) -> None:
    for index in range(5):
        directory = job.output / f"controls/observer-{index}"
        if index == 4:
            directory.mkdir()
            write_json(directory / "actions.json", forbidden_descriptor())
        _ = run(argv(job, index), directory / "command", expected=1)
    validate_observer_controls(job)


def validate_observer_controls(job: BackupSaleRun) -> None:
    for index in range(5):
        directory = job.output / f"controls/observer-{index}"
        expected = (
            "lifecycle observer modes are mutually exclusive"
            if index < 4
            else "backup-enabled sale is outside bridge mode"
        )
        if (
            not exact(
                read_json(directory / "command/argv.json"), list(argv(job, index))
            )
            or not exact(
                read_json(directory / "command/process.json"),
                {"returncode": 1, "expected": 1},
            )
            or expected not in (directory / "command/stderr.log").read_text()
        ):
            raise WorldCheckError("Actual native observer refusal differs")
        if (directory / "forbidden/results.json").exists() or list(
            directory.rglob("*.sav")
        ):
            raise WorldCheckError("Refused observer published gameplay output")
        if index == 4 and not exact(
            read_json(directory / "actions.json"), forbidden_descriptor()
        ):
            raise WorldCheckError("Backup-enabled refusal descriptor changed")
