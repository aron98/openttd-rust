from __future__ import annotations

import os
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Final

from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import sequence, text
from scripts.world_check_support import Json, WorldCheckError, at, run, write_json

SELECTOR = "runtime::ordered_sale_native::original_ordered_sale_case"

CONFIG: Final = """[gui]
autosave = off
autosave_on_exit = false
threaded_saves = false
[misc]
language = english.lng
savegame_format = none
survey_participation = no
"""


def member(root: Path, name: str) -> Path:
    path = root / name
    if (
        Path(name).is_absolute()
        or ".." in Path(name).parts
        or path.is_symlink()
        or not path.resolve().is_relative_to(root.resolve())
    ):
        raise WorldCheckError("Ordered sale path escapes evidence root")
    return path


@dataclass(frozen=True, slots=True)
class OrderedSaleRun:
    root: Path
    output: Path
    oracle: Path
    binaries: Json
    fixtures: Json

    def executable(self, name: str) -> str:
        value = text(at(self.binaries, (name, "executable")))
        if digest(Path(value)) != at(self.binaries, (name, "sha256")):
            raise WorldCheckError("Ordered sale executable changed")
        return value

    def native(self, source: Path, case: Path, descriptor: Json) -> None:
        case.mkdir(parents=True, exist_ok=False)
        actions = case / "actions.json"
        write_json(actions, descriptor)
        native = case / "original"
        native.mkdir()
        config = native / "openttd.cfg"
        _ = config.write_text(CONFIG)
        variables = {
            "OTTD_ORDERED_SALE_OBSERVE": "1",
            "OTTD_ORDER_STATE_PATH": str(native / "loaded.json"),
            "OTTD_WORLD_PATH": str(native / "saved-world.json"),
            "OTTD_WORLD_SCHEMA_PATH": str(native / "saved-schema.json"),
            "OTTD_ORDER_FIXTURE_PATH": str(actions),
            "OTTD_ORDER_FIXTURE_DIR": str(native / "native"),
        }
        argv = [
            str(self.oracle),
            "-x",
            "-c",
            str(config),
            "-snull",
            "-mnull",
            "-d",
            "sl=2",
            "-vnull:ticks=100000000",
            "-g",
            str(source),
        ]
        write_json(native / "argv.json", list(argv))
        write_json(native / "environment.json", dict(variables))
        paths = [self.oracle, source, actions, config]
        before = {str(path): digest(path) for path in paths}
        write_json(native / "bindings-before.json", dict(before))
        environment = {
            key: value
            for key, value in os.environ.items()
            if not key.startswith("OTTD_")
        }
        environment.update(variables)
        result = subprocess.run(
            argv,
            cwd=native,
            env=environment,
            capture_output=True,
            text=True,
            timeout=90,
            check=False,
        )
        _ = (native / "stdout.log").write_text(result.stdout)
        _ = (native / "stderr.log").write_text(result.stderr)
        write_json(
            native / "process.json",
            {"returncode": result.returncode, "expected": 0, "timeout_seconds": 90},
        )
        after = {str(path): digest(path) for path in paths}
        write_json(native / "bindings-after.json", dict(after))
        if before != after or result.returncode != 0:
            raise WorldCheckError("Original ordered sale process or identity failed")
        write_json(
            case / "inputs.json",
            {
                "source": str(source),
                "source_sha256": digest(source),
                "descriptor_sha256": digest(actions),
                "oracle_sha256": digest(self.oracle),
            },
        )

    def prepare(self) -> None:
        results = self.output / "results"
        _ = run(
            [
                "cmake",
                f"-DORACLE={self.oracle}",
                f"-DRUN_DIR={results}/matrix/prepared",
                f"-DINPUT={self.root}/fixtures/replay/clear-v362.sav",
                f"-DCONFIG={self.root}/scripts/reference.cfg",
                "-DPREPARE=ON",
                "-DVECTORS=OFF",
                "-P",
                str(self.root / "scripts/run-depot-runtime-reference.cmake"),
            ],
            results / "prepare-command",
        )
        for entry in sequence(at(self.fixtures, ("jobs",))):
            directory = member(results, text(at(entry, ("directory",))))
            source = member(results, text(at(entry, ("source",))))
            match at(entry, ("kind",)):
                case "native":
                    self.native(source, directory, at(entry, ("descriptor",)))
                case "edit":
                    destination = member(results, text(at(entry, ("output",))))
                    descriptor = destination.with_suffix(".edit.json")
                    descriptor.parent.mkdir(parents=True, exist_ok=True)
                    write_json(descriptor, at(entry, ("edits",)))
                    before = digest(source)
                    _ = run(
                        [
                            self.executable("cli"),
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
                        raise WorldCheckError("Depot fixture edit changed source")
                    write_json(
                        directory / "bindings.json",
                        {
                            "source": str(source),
                            "source_before": before,
                            "source_after": digest(source),
                            "output": str(destination),
                            "output_sha256": digest(destination),
                            "descriptor_sha256": digest(descriptor),
                            "cli_sha256": digest(Path(self.executable("cli"))),
                        },
                    )
                case _:
                    raise WorldCheckError("Unknown bounded depot fixture operation")
