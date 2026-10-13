from __future__ import annotations

import shutil
import subprocess
from copy import deepcopy
from pathlib import Path

from scripts.currency_properties_ci_roster import COMPILER_INPUT, GUARD_INPUT, GUARDS
from scripts.grf_control_evidence import sequence, text
from scripts.language_ci_compare import compare, mapping, number, pointer_path
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    read_json,
    replace,
    write_json,
)


def run_guards(root: Path, output: Path, oracle: Path) -> None:
    base = mapping(read_json(output / "api/rate-boundaries/manifest.json"))
    base.update(mapping(read_json(root / COMPILER_INPUT)))
    inputs = sequence(at(read_json(root / GUARD_INPUT), ("mutations",)))
    compare([text(at(item, ("case",))) for item in inputs], list(GUARDS))
    rows: list[Json] = []
    for raw in inputs:
        item = mapping(raw)
        name = text(item["case"])
        directory = output / "guards" / name
        directory.mkdir(parents=True, exist_ok=False)
        packs = directory / "pack"
        packs.mkdir()
        _ = shutil.copyfile(
            output / "api/rate-boundaries/pack/input.lng", packs / "input.lng"
        )
        _ = shutil.copyfile(
            output / "api/rate-boundaries/config.cfg", directory / "config.cfg"
        )
        manifest = deepcopy(base)
        mapping(at(manifest, ("language",)))["pack_directories"] = [str(packs)]
        if "remove" in item:
            _ = manifest.pop(text(item["remove"]).removeprefix("/"))
        if "pointer" in item:
            pointer = text(item["pointer"])
            replacement = (
                (
                    [at(item, ("repeat", "value"))]
                    * number(at(item, ("repeat", "count")))
                )
                if "repeat" in item
                else item["value"]
            )
            if pointer.count("/") == 1:
                manifest[pointer[1:]] = replacement
            else:
                replace(manifest, pointer_path(manifest, pointer), replacement)
        write_json(directory / "manifest.json", manifest)
        argv = [
            "cmake",
            f"-DORACLE={oracle}",
            f"-DRUN_DIR={directory / 'native'}",
            f"-DMANIFEST={directory / 'manifest.json'}",
            f"-DINPUT={root / 'fixtures/replay/clear-v362.sav'}",
            f"-DCONFIG={directory / 'config.cfg'}",
            "-P",
            str(root / "scripts/check-grf-currency-reference.cmake"),
        ]
        write_json(directory / "argv.json", list(argv))
        process = subprocess.run(
            argv, cwd=root, capture_output=True, text=True, timeout=60, check=False
        )
        _ = (directory / "stdout.log").write_text(process.stdout)
        _ = (directory / "stderr.log").write_text(process.stderr)
        write_json(directory / "status.json", process.returncode)
        native_error = directory / "native/stderr.log"
        errors = process.stderr + (
            native_error.read_text() if native_error.exists() else ""
        )
        if (
            process.returncode == 0
            or text(item["expect"]) not in errors
            or (directory / "native/currency.json").exists()
        ):
            raise WorldCheckError(f"Property guard not observed: {name}")
        rows.append({"case": name, "diagnostic": item["expect"], "passed": True})
    write_json(output / "guards/summary.json", rows)
