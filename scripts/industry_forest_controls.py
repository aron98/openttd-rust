# SPDX-License-Identifier: GPL-2.0-only
"""Corrupt complete forest observations and execute the real comparison harness."""

import json
import shutil
from pathlib import Path

from scripts.gameplay_foundations import digest
from scripts.grf_control_run import ControlRun
from scripts.industry_forest_proof import ROOT, TEST
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    decode_json,
    read_json,
    replace,
    write_json,
)


def copy_case(native: Path, output: Path) -> None:
    output.mkdir(parents=True)
    for source in native.iterdir():
        if source.is_file() and source.suffix in (".sav", ".json", ".jsonl"):
            _ = shutil.copy2(source, output / source.name)


def reject(binary: Path, case: Path, changed: Path) -> None:
    result = ControlRun(ROOT, case, binary).run(
        "reject",
        [str(binary), "--ignored", "--exact", TEST, "--nocapture"],
        {
            "OTTD_INDUSTRY_NATIVE_DIR": str(case),
            "OTTD_INDUSTRY_RUST_OUTPUT": str(case / "rust"),
        },
        expected=101,
    )
    if "assertion" not in result.stderr or TEST not in result.stdout:
        raise WorldCheckError("Forest corruption failed for a different reason")
    write_json(
        case / "PASS.json",
        {
            "rejected": True,
            "binary_sha256": digest(binary),
            "corrupted_sha256": digest(changed),
        },
    )


def controls(binary: Path, native: Path, output: Path) -> None:
    caches: list[tuple[str, tuple[str | int, ...], Json]] = [
        ("membership", ("type_members", 3), []),
        ("increment", ("daily_increment",), 0),
        ("manager", ("builder", "types", 239, "probability"), 1),
    ]
    for name, key, value in caches:
        case = output / name
        copy_case(native, case)
        changed = case / "initial.industries.json"
        observed = read_json(changed)
        replace(observed, key, value)
        write_json(changed, observed)
        reject(binary, case, changed)
    for name, phase, key in [
        ("raw", "tile_loop", ("raw", 2)),
        ("random", "production", ("random", 0)),
        ("stockpile", "production", ("industry", "produced", 0, "waiting")),
    ]:
        case = output / name
        copy_case(native, case)
        changed = case / "industry-events.jsonl"
        rows = [decode_json(line) for line in changed.read_text().splitlines()]
        selected = next(
            row
            for row in rows
            if at(row, ("phase",)) == phase and at(row, ("edge",)) == "leave"
        )
        old = at(selected, key)
        if not isinstance(old, int):
            raise WorldCheckError("Observed industry integer missing")
        replace(selected, key, old ^ 1)
        _ = changed.write_text("\n".join(json.dumps(row) for row in rows) + "\n")
        reject(binary, case, changed)
