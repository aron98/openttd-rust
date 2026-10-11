# SPDX-License-Identifier: GPL-2.0-only
"""Strict receipt, input and scenario accounting for the private house proof."""

from __future__ import annotations

from pathlib import Path

from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import sequence, text
from scripts.house_runtime_controls import production
from scripts.language_ci_compare import number
from scripts.world_check_support import WorldCheckError, at, read_json, write_json


def native_receipts(directory: Path, binary: Path) -> None:
    binding = read_json(directory / "bindings.json")
    if at(binding, ("binary_sha256",)) != digest(binary):
        raise WorldCheckError("Native epoch mismatch")
    for field, filename in [
        ("protocol_sha256", "protocol.json"),
        ("config_sha256", "openttd.cfg"),
    ]:
        if at(binding, (field,)) != digest(directory / filename):
            raise WorldCheckError("Native invocation input drift")
    source = Path(text(at(binding, ("input",))))
    if at(binding, ("input_sha256",)) != digest(source):
        raise WorldCheckError("Native saved input drift")
    process = read_json(directory / "logs/native/process.json")
    if at(process, ("returncode",)) != 0 or at(process, ("expected",)) != 0:
        raise WorldCheckError("Native process did not succeed")
    results = read_json(directory / "results.json")
    for row in sequence(at(results, ("actions",))):
        if at(row, ("op",)) == "command":
            for phase in ("test", "exec", "result"):
                if at(row, ("receipt", phase, "success")) is not True:
                    raise WorldCheckError("Original preparation command failed")


def audit(corpus: Path, proof: Path, binary: Path) -> None:
    counts = {
        "original-6144": 6144,
        "multi-6144": 6144,
        "bitcount-8192": 8192,
        "scale15-8192": 8192,
        "original-year": 27010,
        "paused": 7,
        "zero": 0,
    }
    for directory in sorted(corpus.iterdir()):
        if (directory / "bindings.json").is_file():
            native_receipts(directory, binary)
    for name, count in counts.items():
        result = read_json(proof / name / "PASS.json")
        if at(result, ("calls",)) != count:
            raise WorldCheckError("Incomplete named house horizon")
        if count > 7:
            _ = production(corpus / name)
    year = read_json(corpus / "original-year/final.runtime.json")
    if (
        at(year, ("after_save", "calendar_year")) != 2101
        or at(year, ("after_save", "economy_year")) != 2101
    ):
        raise WorldCheckError("Missing genuine year boundary")
    for origin in ("native-origin", "rust-origin"):
        result = read_json(proof / (origin + "-rust") / "PASS.json")
        if at(result, ("calls",)) != 4096:
            raise WorldCheckError("Incomplete saved continuation")
    write_json(
        proof / "audit.json",
        {
            "native_binary_sha256": digest(binary),
            "primary_complete_calls": sum(counts.values()),
            "saved_continuation_complete_calls": 8192,
            "primary_boundary_count": sum(
                number(
                    at(
                        read_json(proof / name / "PASS.json"),
                        ("complete_world_derived_cache_boundaries",),
                    )
                )
                for name in counts
            ),
            "native_command_receipts_success": True,
            "input_protocol_config_process_bindings": True,
            "year": 2101,
        },
    )
