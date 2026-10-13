"""Execute native road-stop commands and compare owned runtime and complete saves."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from scripts.backup_sale_run import CONFIG
from scripts.gameplay_foundations import digest, require_test
from scripts.grf_control_evidence import sequence, text
from scripts.owned_restore_projection import project
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    read_json,
    run,
    write_json,
)

SELECTOR = "runtime::stations::native::original_road_station_case"


@dataclass(frozen=True, slots=True)
class StationPair:
    oracle: Path
    cli: Path
    runner: Path

    def execute(
        self, source: Path, case: Path, plan: Json, outcomes: tuple[bool, ...]
    ) -> None:
        case.mkdir(parents=True, exist_ok=False)
        write_json(case / "actions.json", plan)
        _ = (case / "openttd.cfg").write_text(CONFIG)
        inputs = (source, self.oracle, self.cli, self.runner, case / "actions.json")
        before: Json = {str(path): digest(path) for path in inputs}
        write_json(case / "bindings-before.json", before)
        native = case / "native"
        _ = run(
            [
                "env",
                "OTTD_OWNED_RESTORE_OBSERVE=1",
                "OTTD_ROAD_CYCLE_PREPARE=1",
                "OTTD_ROAD_STATION_OBSERVE=1",
                f"OTTD_ORDER_STATE_PATH={case}/loaded.json",
                f"OTTD_ORDER_FIXTURE_PATH={case}/actions.json",
                f"OTTD_ORDER_FIXTURE_DIR={native}",
                str(self.oracle),
                "-x",
                "-c",
                str(case / "openttd.cfg"),
                "-snull",
                "-mnull",
                "-vnull:ticks=100000000",
                "-g",
                str(source),
            ],
            case / "native-command",
        )
        commands = [
            a
            for a in sequence(at(read_json(native / "results.json"), ("actions",)))
            if at(a, ("input", "op")) == "command"
        ]
        if len(commands) != len(outcomes):
            raise WorldCheckError("Station expected command membership differs")
        for action, expected in zip(commands, outcomes, strict=True):
            if at(action, ("result", "receipt", "posted")) is not expected:
                raise WorldCheckError("Original station command outcome differs")
            if (
                not expected or at(action, ("input", "request", "mode")) == "estimate"
            ) and at(action, ("before",)) != at(action, ("after",)):
                raise WorldCheckError(
                    "Original rejected/estimated station changed runtime"
                )
        rust = case / "rust"
        result = run(
            [
                "env",
                f"ROAD_STATION_INPUT={source}",
                f"ROAD_STATION_ACTIONS={case}/actions.json",
                f"ROAD_STATION_OUTPUT={rust}",
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
            project(read_json(native / "results.json"), plan),
        )
        self.compare(
            case / "projected-native.json",
            rust / "results.json",
            case / "runtime-compare",
        )
        labels = [
            text(at(a, ("label",)))
            for a in sequence(at(plan, ("actions",)))
            if at(a, ("op",)) == "save"
        ]
        if not labels:
            raise WorldCheckError("Station proof requires boundary saves")
        for label in labels:
            self.compare(
                native / f"{label}.derived.json",
                rust / f"{label}.derived.json",
                case / f"{label}-derived-compare",
            )
            exported = run(
                [
                    str(self.cli),
                    "world",
                    str(native / f"{label}.sav"),
                    "--view",
                    "saved",
                ],
                case / f"{label}-native-export",
            )
            native_world = case / f"{label}-native.json"
            _ = native_world.write_text(exported.stdout)
            self.compare(
                native_world,
                rust / f"{label}.world.json",
                case / f"{label}-world-compare",
            )
            decoded = run(
                [str(self.cli), "world", str(rust / f"{label}.sav"), "--view", "saved"],
                case / f"{label}-rust-export",
            )
            rust_world = case / f"{label}-decoded.json"
            _ = rust_world.write_text(decoded.stdout)
            self.compare(native_world, rust_world, case / f"{label}-save-compare")
        after: Json = {str(path): digest(path) for path in inputs}
        write_json(case / "bindings-after.json", after)
        if before != after:
            raise WorldCheckError("Station proof execution inputs changed")
        write_json(
            case / "PASS.json",
            {"boundary_saves": len(labels), "runtime_receipts_rng_world": True},
        )

    def compare(self, expected: Path, actual: Path, log: Path) -> None:
        _ = run([str(self.cli), "compare", str(expected), str(actual)], log)
