# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through: bash scripts/check-worlds.sh
from __future__ import annotations

import json
import subprocess
from collections.abc import Callable
from pathlib import Path
from typing import TypeAlias

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
