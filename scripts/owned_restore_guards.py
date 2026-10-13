from __future__ import annotations

from itertools import combinations
from typing import Final

from scripts.backup_sale_guards import forbidden_descriptor
from scripts.context_ci_support import exact
from scripts.gameplay_foundations import digest
from scripts.owned_restore_run import RestoreRun
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    read_json,
    run,
    write_json,
)

MODES: Final = (
    "OTTD_DEPOT_REMOVAL_OBSERVE",
    "OTTD_ORDERED_SALE_OBSERVE",
    "OTTD_BACKUP_SALE_OBSERVE",
    "OTTD_BACKUP_ENABLED_SALE_OBSERVE",
    "OTTD_OWNED_RESTORE_OBSERVE",
)


def cases() -> list[tuple[str, list[str], str, Json]]:
    result: list[tuple[str, list[str], str, Json]] = []
    for count in range(2, 6):
        for chosen in combinations(MODES, count):
            name = f"mixed-{len(result)}"
            result.append(
                (
                    name,
                    [f"{mode}=1" for mode in chosen],
                    "lifecycle observer modes are mutually exclusive",
                    {
                        "schema_version": 1,
                        "case": name,
                        "role": "sp",
                        "exit": True,
                        "actions": [{"op": "snapshot", "label": "forbidden"}],
                    },
                )
            )
    for index, mode in enumerate(MODES[:4]):
        name = f"old-mode-{index}-build-refusal"
        result.append(
            (
                name,
                [f"{mode}=1"],
                "unsupported lifecycle fixture command",
                {
                    "schema_version": 1,
                    "case": name,
                    "role": "sp",
                    "exit": True,
                    "actions": [
                        {
                            "op": "command",
                            "request": {
                                "company": 0,
                                "mode": "post",
                                "command": {
                                    "kind": "build_vehicle",
                                    "tile": 1560,
                                    "engine": 116,
                                    "cargo": 255,
                                    "use_free_vehicles": False,
                                    "client_id": 0,
                                },
                            },
                        }
                    ],
                },
            )
        )
    result.append(
        (
            "false-bridge-true",
            ["OTTD_BACKUP_SALE_OBSERVE=1"],
            "backup-enabled sale is outside bridge mode",
            forbidden_descriptor(),
        )
    )
    return result


def argv(job: RestoreRun, name: str, variables: list[str]) -> list[str]:
    directory = job.output / "controls/native" / name
    return [
        "env",
        *variables,
        f"OTTD_ORDER_STATE_PATH={directory}/loaded.json",
        f"OTTD_ORDER_FIXTURE_PATH={directory}/actions.json",
        f"OTTD_ORDER_FIXTURE_DIR={directory}/forbidden",
        str(job.oracle),
        "-x",
        "-c",
        str(job.output / "results/cases/owned-baseline/original/openttd.cfg"),
        "-snull",
        "-mnull",
        "-vnull:ticks=100000000",
        "-g",
        str(job.output / "results/matrix/copies.sav"),
    ]


def observer_controls(job: RestoreRun) -> None:
    for name, variables, _, descriptor in cases():
        directory = job.output / "controls/native" / name
        directory.mkdir(parents=True)
        write_json(directory / "actions.json", descriptor)
        inputs = (
            job.oracle,
            job.output / "results/matrix/copies.sav",
            directory / "actions.json",
        )
        before = {str(path): digest(path) for path in inputs}
        write_json(directory / "bindings-before.json", dict(before))
        _ = run(argv(job, name, variables), directory / "command", expected=1)
        write_json(
            directory / "bindings-after.json",
            {str(path): digest(path) for path in inputs},
        )
    validate_observer_controls(job)


def validate_observer_controls(job: RestoreRun) -> None:
    for name, variables, expected, descriptor in cases():
        directory = job.output / "controls/native" / name
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
            raise WorldCheckError("Native Restore refusal differs")
        expected_bindings: Json = {
            str(path): digest(path)
            for path in (
                job.oracle,
                job.output / "results/matrix/copies.sav",
                directory / "actions.json",
            )
        }
        if not exact(
            read_json(directory / "bindings-before.json"), expected_bindings
        ) or not exact(read_json(directory / "bindings-after.json"), expected_bindings):
            raise WorldCheckError("Restore native guard input identity differs")
        if (directory / "forbidden/results.json").exists() or list(
            directory.rglob("*.sav")
        ):
            raise WorldCheckError("Refused Restore observer published gameplay")
