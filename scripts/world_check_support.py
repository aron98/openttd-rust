# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through: bash scripts/check-worlds.sh
from __future__ import annotations

import copy
import hashlib
import json
import subprocess
from collections.abc import Callable
from pathlib import Path
from typing import Protocol, TypeAlias

Json: TypeAlias = bool | int | float | str | list["Json"] | dict[str, "Json"] | None
decode_json: Callable[[str], Json] = json.loads
ROOT = Path(__file__).resolve().parents[1]


class WorldCheckError(RuntimeError):
    pass


def write_json(path: Path, value: Json) -> None:
    _ = path.write_text(json.dumps(value, indent=2) + "\n")


def read_json(path: Path) -> Json:
    return decode_json(path.read_text())


def run(argv: list[str], directory: Path, expected: int = 0) -> subprocess.CompletedProcess[str]:
    directory.mkdir(parents=True, exist_ok=False)
    write_json(directory / "argv.json", [argument for argument in argv])
    result = subprocess.run(argv, capture_output=True, text=True, timeout=65, check=False, cwd=ROOT)
    _ = (directory / "stdout.log").write_text(result.stdout)
    _ = (directory / "stderr.log").write_text(result.stderr)
    write_json(directory / "process.json", {"returncode": result.returncode, "expected": expected})
    if result.returncode != expected:
        raise WorldCheckError(f"Expected exit {expected}, got {result.returncode}: {directory}")
    return result


def at(value: Json, path: tuple[str | int, ...]) -> Json:
    for part in path:
        match part:
            case str():
                if not isinstance(value, dict):
                    raise WorldCheckError(f"Expected object at {part}")
                value = value[part]
            case int():
                if not isinstance(value, list):
                    raise WorldCheckError(f"Expected list at {part}")
                value = value[part]
    return value


def replace(value: Json, path: tuple[str | int, ...], replacement: Json) -> None:
    parent = at(value, path[:-1])
    match path[-1]:
        case str() as key:
            if not isinstance(parent, dict):
                raise WorldCheckError("Expected object mutation target")
            parent[key] = replacement
        case int() as index:
            if not isinstance(parent, list):
                raise WorldCheckError("Expected list mutation target")
            parent[index] = replacement


class WorldDriver(Protocol):
    @property
    def cli(self) -> Path: ...
    @property
    def oracle(self) -> Path: ...
    @property
    def artifacts(self) -> Path: ...
    def native(self, save: Path, case: Path, *, modded: bool = False) -> Path: ...
    def export(self, save: Path, view: str, output: Path, log: Path) -> None: ...
    def compare(self, expected: Path, actual: Path, log: Path) -> None: ...


def resaved_checkpoint(driver: WorldDriver, native: Path, case: Path, *, modded: bool = False) -> None:
    save = native / "save/autosave/exit.sav"
    reloaded = driver.native(save, case, modded=modded)
    driver.export(save, "derived", case / "rust-derived.json", case / "derived-command")
    driver.compare(reloaded / "derived.json", case / "rust-derived.json", case / "derived-compare")
    write_json(case / "input.json", {"checkpoint": "native-resaved exit", "save": str(save),
        "sha256": hashlib.sha256(save.read_bytes()).hexdigest(), "native_input": str(save), "rust_input": str(save)})


def content_mutation(driver: WorldDriver, name: str, save: Path, baseline: Path, *, modded: bool = False) -> Path:
    case = driver.artifacts / name / "mutation"
    case.mkdir()
    driver.export(save, "saved", case / "before.json", case / "before-command")
    before = read_json(case / "before.json")
    path = ("chunks", "CITY", "records", "0", "name")
    old = at(before, path)
    value: Json = list(f"Edited {name} town".encode())
    if old == value:
        raise WorldCheckError(f"Active-content mutation did not change {name}")
    script = "AIPL" if modded else "GSDT"
    data = at(before, ("chunks", script, "records", "0", "script_data"))
    if not isinstance(data, list) or len(data) < 20:
        raise WorldCheckError(f"Active-content fixture has no meaningful {script} state")
    write_json(case / "edits.json", {"schema_version": 1, "edits": [
        {"kind": "field", "chunk": "CITY", "record": 0, "path": ["name"], "value": {"bytes": value}}]})
    edited = case / "edited.sav"
    _ = run([str(driver.cli), "edit-world", str(save), str(case / "edits.json"), str(edited)], case / "edit-command")
    driver.export(edited, "saved", case / "edited-saved.json", case / "edited-saved-command")
    native = driver.native(edited, case, modded=modded)
    driver.export(edited, "derived", case / "rust-derived.json", case / "derived-command")
    driver.compare(native / "derived.json", case / "rust-derived.json", case / "derived-compare")
    driver.export(native / "save/autosave/exit.sav", "saved", case / "rust-saved.json", case / "saved-command")
    driver.compare(native / "world.json", case / "rust-saved.json", case / "saved-compare")
    expected = copy.deepcopy(read_json(baseline / "world.json"))
    replace(expected, path, value)
    write_json(case / "expected-native.json", expected)
    driver.compare(case / "expected-native.json", native / "world.json", case / "change-compare")
    resaved_checkpoint(driver, native, case / "resaved", modded=modded)
    recipe = "check-world-builder.cmake" if modded else "check-world-game.cmake"
    for label, source, log in [("script-baseline", baseline, "script-baseline-command"), ("script-reload", native, "script-command")]:
        command = ["cmake", f"-DORACLE={driver.oracle}", f"-DRUN_DIR={case / label}",
                   f"-DINPUT={source / 'save/autosave/exit.sav'}", "-DRELOAD=ON"]
        if modded:
            command.append("-DMODDED=ON")
        _ = run([*command, "-P", str(ROOT / "scripts" / recipe)], case / log)
    restored = case / "script-reload"
    resaved_checkpoint(driver, restored, case / "script-resaved", modded=modded)
    resaved_checkpoint(driver, case / "script-baseline", case / "script-baseline-resaved", modded=modded)
    restored_world = read_json(restored / "world.json")
    expected_restored = read_json(case / "script-baseline/world.json")
    replace(expected_restored, path, value)
    write_json(case / "restored-expected-native.json", expected_restored)
    driver.compare(case / "restored-expected-native.json", restored / "world.json", case / "restored-change-compare")
    driver.export(restored / "save/autosave/exit.sav", "saved", case / "restored-rust-saved.json", case / "restored-saved-command")
    driver.compare(restored / "world.json", case / "restored-rust-saved.json", case / "restored-saved-compare")
    for label, source in [("before", before), ("after", read_json(case / "edited-saved.json")),
                          ("native", read_json(native / "world.json")), ("restored", restored_world),
                          ("native-baseline", read_json(baseline / "world.json")),
                          ("restored-baseline", read_json(case / "script-baseline/world.json"))]:
        write_json(case / f"content-{label}.json", {chunk: at(source, ("chunks", chunk)) for chunk in ("NGRF", "AIPL", "GSDT", "PSAC")})
    checkpoints: list[Json] = []
    for reference, label, log in [("before", "after", "content-compare"), ("native-baseline", "native", "content-native-compare"),
                                  ("restored-baseline", "restored", "content-restored-compare")]:
        expected_content, actual_content = case / f"content-{reference}.json", case / f"content-{label}.json"
        driver.compare(expected_content, actual_content, case / log)
        checkpoints.append({"expected": str(expected_content), "actual": str(actual_content), "comparison": str(case / log)})
    write_json(case / "content-checkpoints.json", checkpoints)
    write_json(case / "assertion.json", {"path": list(path), "before": old, "after": value,
        "preserved_chunks": ["NGRF", "AIPL", "GSDT", "PSAC"], "active_script_chunk": script})
    return edited
