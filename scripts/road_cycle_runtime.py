"""Owned runtime road-order comparisons with original command and save witnesses."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from scripts.backup_sale_run import CONFIG
from scripts.depot_removal_pair import projection
from scripts.gameplay_foundations import digest, require_test
from scripts.grf_control_evidence import sequence, text
from scripts.owned_restore_run import SELECTOR
from scripts.road_cycle_orders import portfolio
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    read_json,
    run,
    write_json,
)


def descriptor(start: int = 0) -> Json:
    actions: list[Json] = []
    for action in sequence(at(portfolio(), ("actions",)))[start:]:
        match at(action, ("op",)):
            case "command":
                actions.append({"op": "command", "request": at(action, ("request",))})
            case "checkpoint":
                actions.append({"op": "save", "label": at(action, ("label",))})
            case _:
                raise WorldCheckError("Unexpected order portfolio operation")
    return {
        "schema_version": 1,
        "case": "road-orders",
        "role": "sp",
        "exit": True,
        "actions": actions,
    }


@dataclass(frozen=True, slots=True)
class RuntimeProof:
    root: Path
    oracle: Path
    cli: Path
    runner: Path

    def native(self, source: Path, case: Path) -> Path:
        output = case / "original"
        output.mkdir()
        config = output / "openttd.cfg"
        _ = config.write_text(CONFIG)
        args = [
            "env",
            "OTTD_OWNED_RESTORE_OBSERVE=1",
            "OTTD_ROAD_CYCLE_PREPARE=1",
            f"OTTD_ORDER_STATE_PATH={output}/loaded.json",
            f"OTTD_WORLD_PATH={output}/saved-world.json",
            f"OTTD_WORLD_SCHEMA_PATH={output}/saved-schema.json",
            f"OTTD_ORDER_FIXTURE_PATH={case}/actions.json",
            f"OTTD_ORDER_FIXTURE_DIR={output}/native",
            str(self.oracle),
            "-x",
            "-c",
            str(config),
            "-snull",
            "-mnull",
            "-vnull:ticks=100000000",
            "-g",
            str(source),
        ]
        _ = run(args, case / "native-command")
        return output / "native"

    def pair(self, source: Path, case: Path, plan: Json) -> None:
        case.mkdir(parents=True, exist_ok=False)
        write_json(case / "actions.json", plan)
        inputs = (source, self.oracle, self.cli, self.runner, case / "actions.json")
        before: Json = {str(path): digest(path) for path in inputs}
        write_json(case / "bindings-before.json", before)
        native = self.native(source, case)
        rust = case / "rust"
        result = run(
            [
                "env",
                f"OWNED_RESTORE_INPUT={source}",
                f"OWNED_RESTORE_ACTIONS={case}/actions.json",
                f"OWNED_RESTORE_OUTPUT={rust}",
                str(self.runner),
                "--exact",
                SELECTOR,
                "--ignored",
                "--nocapture",
            ],
            case / "rust-command",
        )
        require_test(result.stdout, SELECTOR)
        write_json(
            case / "projected-native.json",
            projection(read_json(native / "results.json"), plan),
        )
        _ = run(
            [
                str(self.cli),
                "compare",
                str(case / "projected-native.json"),
                str(rust / "results.json"),
            ],
            case / "runtime-compare",
        )
        labels = [
            text(at(action, ("label",)))
            for action in sequence(at(plan, ("actions",)))
            if at(action, ("op",)) == "save"
        ]
        if not labels:
            raise WorldCheckError("Order runtime proof requires saved checkpoints")
        for label in labels:
            result = run(
                [
                    str(self.cli),
                    "world",
                    str(native / f"{label}.sav"),
                    "--view",
                    "saved",
                ],
                case / f"{label}-native-export",
            )
            path = case / f"{label}-native.json"
            _ = path.write_text(result.stdout)
            _ = run(
                [
                    str(self.cli),
                    "compare",
                    str(path),
                    str(rust / f"{label}.world.json"),
                ],
                case / f"{label}-world-compare",
            )
            result = run(
                [str(self.cli), "world", str(rust / f"{label}.sav"), "--view", "saved"],
                case / f"{label}-rust-export",
            )
            decoded = case / f"{label}-decoded.json"
            _ = decoded.write_text(result.stdout)
            _ = run(
                [str(self.cli), "compare", str(path), str(decoded)],
                case / f"{label}-saved-compare",
            )
        after: Json = {str(path): digest(path) for path in inputs}
        write_json(case / "bindings-after.json", after)
        if before != after:
            raise WorldCheckError("Order proof inputs or executables changed")
        write_json(
            case / "PASS.json", {"checkpoints": len(labels), "runtime_and_saved": True}
        )
