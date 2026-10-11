# SPDX-License-Identifier: GPL-2.0-only
"""Actual comparator rejection for corrupted original industry-manager observations."""

import json
import shutil
from pathlib import Path

from scripts.gameplay_foundations import digest
from scripts.grf_control_run import ControlRun
from scripts.industry_manager_evidence import manager_cache
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    decode_json,
    read_json,
    replace,
    write_json,
)

TEST = "native_house_lifecycle_matches_complete_runtime"


def cache_controls(native: Path, output: Path) -> None:
    world = read_json(native / "final.world.json")
    baseline = read_json(native / "final.industries.json")
    manager_cache(world, baseline)
    cases: list[tuple[str, tuple[str | int, ...], Json]] = [
        ("probability", ("builder", "types", 239, "probability"), 1),
        ("wait", ("builder", "types", 0, "wait_count"), 99),
        ("membership", ("type_members", 3), [0]),
        ("increment", ("daily_increment",), 0),
    ]
    for name, path, value in cases:
        observed = read_json(native / "final.industries.json")
        replace(observed, path, value)
        write_json(output / f"{name}-input.json", observed)
        try:
            manager_cache(world, observed)
        except WorldCheckError as error:
            write_json(output / f"{name}-PASS.json", {"rejected": str(error)})
        else:
            raise WorldCheckError(f"Industry cache corruption accepted: {name}")


def loop_controls(native: Path, binary: Path, output: Path) -> None:
    root = Path(__file__).resolve().parents[1]
    for kind in ("world", "random", "cursor"):
        case = output / kind
        case.mkdir()
        names = [
            "initial.sav",
            "protocol.json",
            "initial.house-specs.json",
            "world-events.jsonl",
        ]
        names += [
            f"{label}.{view}.json"
            for label in ("initial", "final")
            for view in ("world", "derived", "houses")
        ]
        for name in names:
            _ = shutil.copy2(native / name, case / name)
        if (native / "events.jsonl").is_file():
            _ = shutil.copy2(native / "events.jsonl", case / "events.jsonl")
        if kind == "world":
            path = case / "final.world.json"
            value = read_json(path)
            replace(
                value,
                ("chunks", "ECMY", "records", "0", "industry_daily_change_counter"),
                69,
            )
            write_json(path, value)
        else:
            path = case / "world-events.jsonl"
            rows = [decode_json(line) for line in path.read_text().splitlines()]
            field = ("random", 0) if kind == "random" else ("cursor",)
            current = at(rows[1], field)
            if not isinstance(current, int):
                raise WorldCheckError("Invalid observed event integer")
            replace(rows[1], field, current ^ 1)
            _ = path.write_text(
                "\n".join(json.dumps(row) for row in rows) + "\n"
            )
        result = ControlRun(root, case, binary).run(
            "reject",
            [str(binary), "--ignored", "--exact", TEST, "--nocapture"],
            {
                "OTTD_HOUSE_NATIVE_DIR": str(case),
                "OTTD_HOUSE_RUST_OUTPUT": str(case / "rust"),
            },
            expected=101,
        )
        if "assertion" not in result.stderr or TEST not in result.stdout:
            raise WorldCheckError(
                "Industry loop corruption rejected for another reason"
            )
        write_json(
            case / "PASS.json",
            {
                "binary_sha256": digest(binary),
                "corrupted_sha256": digest(path),
                "rejected": True,
            },
        )
