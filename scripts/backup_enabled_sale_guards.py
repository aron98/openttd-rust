from __future__ import annotations

from itertools import combinations
from typing import Final

from scripts.backup_sale_guards import forbidden_descriptor
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
    "OTTD_BACKUP_ENABLED_SALE_OBSERVE",
)


def cases() -> list[tuple[str, list[str], str, Json]]:
    result: list[tuple[str, list[str], str, Json]] = []
    for count in (2, 3, 4):
        for chosen in combinations(MODES, count):
            name = f"mixed-{len(result)}"
            descriptor: Json = {
                "schema_version": 1,
                "case": name,
                "role": "sp",
                "exit": True,
                "actions": [{"op": "snapshot", "label": "forbidden"}],
            }
            result.append(
                (
                    name,
                    [f"{mode}=1" for mode in chosen],
                    "lifecycle observer modes are mutually exclusive",
                    descriptor,
                )
            )
    result.append(
        (
            "old-bridge-true",
            ["OTTD_BACKUP_SALE_OBSERVE=1"],
            "backup-enabled sale is outside bridge mode",
            forbidden_descriptor(),
        )
    )
    result.append(
        (
            "invalid-enabled-mode",
            ["OTTD_BACKUP_ENABLED_SALE_OBSERVE=2"],
            "invalid backup enabled sale observer mode",
            {
                "schema_version": 1,
                "case": "invalid-enabled-mode",
                "role": "sp",
                "exit": True,
                "actions": [{"op": "snapshot", "label": "forbidden"}],
            },
        )
    )
    result.append(
        (
            "forbidden-build",
            ["OTTD_BACKUP_ENABLED_SALE_OBSERVE=1"],
            "unsupported lifecycle fixture command",
            {
                "schema_version": 1,
                "case": "forbidden-build",
                "role": "sp",
                "exit": True,
                "actions": [
                    {
                        "op": "command",
                        "request": {
                            "company": 0,
                            "mode": "post",
                            "command": {
                                "kind": "build_road_depot",
                                "tile": 1560,
                                "road_type": 0,
                                "direction": 0,
                            },
                        },
                    }
                ],
            },
        )
    )
    return result


def argv(job: BackupSaleRun, name: str, variables: list[str]) -> list[str]:
    directory = job.output / "controls" / name
    return [
        "env",
        *variables,
        f"OTTD_ORDER_STATE_PATH={directory}/loaded.json",
        f"OTTD_ORDER_FIXTURE_PATH={directory}/actions.json",
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
    for name, variables, _, descriptor in cases():
        directory = job.output / "controls" / name
        directory.mkdir()
        write_json(directory / "actions.json", descriptor)
        _ = run(argv(job, name, variables), directory / "command", expected=1)
    validate_observer_controls(job)


def validate_observer_controls(job: BackupSaleRun) -> None:
    for name, variables, expected, descriptor in cases():
        directory = job.output / "controls" / name
        if (
            not exact(read_json(directory / "actions.json"), descriptor)
            or not exact(
                read_json(directory / "command/argv.json"),
                list(argv(job, name, variables)),
            )
            or not exact(
                read_json(directory / "command/process.json"),
                {"returncode": 1, "expected": 1},
            )
            or expected not in (directory / "command/stderr.log").read_text()
        ):
            raise WorldCheckError("Native enabled-sale refusal differs")
        if (directory / "forbidden/results.json").exists() or list(
            directory.rglob("*.sav")
        ):
            raise WorldCheckError("Refused lifecycle observer published gameplay")
