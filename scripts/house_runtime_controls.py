# SPDX-License-Identifier: GPL-2.0-only
"""Proof corruption controls and authentic producer event accounting."""

from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path

from scripts.grf_control_evidence import sequence
from scripts.grf_control_run import ControlRun
from scripts.house_runtime_proof import ROOT, TEST, HouseProof
from scripts.language_ci_compare import number
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    decode_json,
    read_json,
    replace,
    write_json,
)


def events(directory: Path) -> list[Json]:
    return [
        decode_json(line)
        for line in (directory / "events.jsonl").read_text().splitlines()
    ]


def production(directory: Path) -> Json:
    rows = events(directory)
    totals: dict[int, int] = {}
    construction: dict[int, int] = {}
    for before, after in zip(rows[::2], rows[1::2], strict=True):
        tile = number(at(before, ("tile",)))
        if number(at(before, ("raw", 4))) & 128 == 0:
            construction[tile] = construction.get(tile, 0) + 1
        previous = {
            number(at(row, ("cargo",))): number(at(row, ("production",)))
            for row in sequence(at(before, ("supplied",)))
        }
        for row in sequence(at(after, ("supplied",))):
            cargo = number(at(row, ("cargo",)))
            delta = number(at(row, ("production",))) - previous.get(cargo, 0)
            if delta < 0 or number(at(row, ("transported",))) != 0:
                raise WorldCheckError("Unexpected callback history or recipient")
            totals[cargo] = totals.get(cargo, 0) + delta
    if totals.get(0, 0) <= 0 or totals.get(2, 0) <= 0:
        raise WorldCheckError("Missing genuine passenger or mail production")
    if construction.get(2137) != 24:
        raise WorldCheckError(
            "Missing full original single-tile construction lifecycle"
        )
    if 1060 in construction and construction[1060] != 24:
        raise WorldCheckError("Missing full original multi-tile construction lifecycle")
    return {
        "callback_pairs": len(rows) // 2,
        "production": {str(cargo): count for cargo, count in sorted(totals.items())},
        "unfinished_callbacks": {
            str(tile): count for tile, count in sorted(construction.items())
        },
    }


def corruptions(proof: HouseProof, native: Path, output: Path) -> None:
    rows = events(native)
    alterations: list[tuple[str, str, Json]] = []
    for name, path in [
        ("random", ("random", 0)),
        ("raw", ("footprint", 0, "raw", 6)),
    ]:
        altered = deepcopy(rows)
        replace(altered[1], path, number(at(altered[1], path)) ^ 1)
        alterations.append((name, "events.jsonl", altered))
    alterations.append(("missing-event", "events.jsonl", rows[2:]))
    for name, file, path in [
        ("cache", "initial.houses.json", ("global_id_count", 0)),
        ("catalog", "initial.house-specs.json", (0, "population")),
    ]:
        value = read_json(native / file)
        replace(value, path, number(at(value, path)) ^ 1)
        alterations.append((name, file, value))
    for name, file, value in alterations:
        fixture = output / name
        fixture.mkdir(parents=True)
        for source in native.iterdir():
            if source.is_file() and source.name != file:
                (fixture / source.name).symlink_to(source)
        if file.endswith("jsonl"):
            _ = (fixture / file).write_text(
                "".join(json.dumps(row) + "\n" for row in sequence(value)),
            )
        else:
            write_json(fixture / file, value)
        result = ControlRun(ROOT, output / (name + "-logs"), proof.lifecycle).run(
            "corruption",
            [str(proof.lifecycle), "--ignored", "--exact", TEST, "--nocapture"],
            {
                "OTTD_HOUSE_NATIVE_DIR": str(fixture),
                "OTTD_HOUSE_RUST_OUTPUT": str(output / (name + "-rust")),
            },
            expected=101,
        )
        if "assertion" not in result.stderr or "1 failed" not in result.stdout:
            raise WorldCheckError("Corruption did not reach a semantic assertion")
    write_json(output / "PASS.json", {"rejected": [row[0] for row in alterations]})
