from __future__ import annotations

import os
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Final

from scripts.backup_sale_run import CONFIG
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import text
from scripts.world_check_support import Json, WorldCheckError, at, write_json

SELECTOR: Final = "runtime::owned_restore_native::original_owned_restore_case"
MALLOC: Final = (
    "MallocScribble",
    "MallocPreScribble",
    "MallocNanoZone",
    "MALLOC_PERTURB_",
)


@dataclass(frozen=True, slots=True)
class RestoreRun:
    root: Path
    output: Path
    oracle: Path
    binaries: Json
    fixtures: Json

    def executable(self, name: str) -> str:
        value = text(at(self.binaries, (name, "executable")))
        if digest(Path(value)) != at(self.binaries, (name, "sha256")):
            raise WorldCheckError("Restore executable changed")
        return value

    @property
    def selector(self) -> str:
        return SELECTOR

    @property
    def input_prefix(self) -> str:
        return "OWNED_RESTORE"

    @property
    def observer_mode(self) -> str:
        return "OTTD_OWNED_RESTORE_OBSERVE"

    def native(self, source: Path, case: Path, descriptor: Json) -> None:
        case.mkdir(parents=True, exist_ok=False)
        actions = case / "actions.json"
        write_json(actions, descriptor)
        native = case / "original"
        native.mkdir()
        config = native / "openttd.cfg"
        _ = config.write_text(CONFIG)
        variables: dict[str, str | None] = {
            self.observer_mode: "1",
            "OTTD_ORDER_STATE_PATH": str(native / "loaded.json"),
            "OTTD_WORLD_PATH": str(native / "saved-world.json"),
            "OTTD_WORLD_SCHEMA_PATH": str(native / "saved-schema.json"),
            "OTTD_ORDER_FIXTURE_PATH": str(actions),
            "OTTD_ORDER_FIXTURE_DIR": str(native / "native"),
            **dict.fromkeys(MALLOC),
        }
        environment = {
            key: value
            for key, value in os.environ.items()
            if not key.startswith(("OTTD_", "OWNED_RESTORE_")) and key not in MALLOC
        }
        environment.update(
            {key: value for key, value in variables.items() if value is not None}
        )
        argv = [
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
        write_json(native / "argv.json", list(argv))
        write_json(native / "environment.json", dict(variables))
        paths = (self.oracle, source, actions, config)
        before = {str(path): digest(path) for path in paths}
        write_json(native / "bindings-before.json", dict(before))
        result = subprocess.run(
            argv,
            cwd=native,
            env=environment,
            capture_output=True,
            timeout=180,
            check=False,
        )
        _ = (native / "stdout.log").write_bytes(result.stdout)
        _ = (native / "stderr.log").write_bytes(result.stderr)
        write_json(
            native / "process.json",
            {"returncode": result.returncode, "expected": 0, "timeout_seconds": 180},
        )
        after = {str(path): digest(path) for path in paths}
        write_json(native / "bindings-after.json", dict(after))
        if before != after or result.returncode != 0:
            raise WorldCheckError("Original Restore process or identity failed")
        write_json(
            case / "inputs.json",
            {
                "source": str(source),
                "source_sha256": digest(source),
                "descriptor_sha256": digest(actions),
                "oracle_sha256": digest(self.oracle),
            },
        )
