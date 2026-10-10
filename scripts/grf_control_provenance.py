from __future__ import annotations

from pathlib import Path

from scripts.gameplay_foundations import digest, require_test
from scripts.grf_control_evidence import text
from scripts.world_check_support import WorldCheckError, at, read_json


def verify(root: Path, output: Path) -> None:
    provenance = read_json(output / "provenance.json")
    hashes = at(provenance, ("source_hashes",))
    if not isinstance(hashes, dict) or not hashes:
        raise WorldCheckError("Missing loader source hashes")
    for name, expected in hashes.items():
        if digest(root / name) != expected:
            raise WorldCheckError(f"Loader source changed during execution: {name}")
    oracle = Path(text(at(provenance, ("native_binary",))))
    if digest(oracle) != at(provenance, ("native_sha256",)):
        raise WorldCheckError("Native loader executable changed during execution")
    binaries = read_json(output / "test-binaries.json")
    for name in ("grf_load_control", "native_grf_control"):
        if digest(output / "bin" / name) != at(binaries, (name, "sha256")):
            raise WorldCheckError("Executed loader test binary changed")
        if (
            at(binaries, (name, "kind")) != ["test"]
            or at(binaries, (name, "profile_test")) is not True
        ):
            raise WorldCheckError("Loader executable is not a Cargo test target")
    for name in (
        "native_load_control_matrix",
        "generated_control_programs_are_admitted",
        "cumulative_limits_refuse_work_without_fabricating_disabled_reports",
        "exact_work_limits_admit_the_last_unit_and_reject_the_next",
        "trace_limit_charges_the_emitted_event_storage",
    ):
        directory = output / "logs" / name
        require_test((directory / "stdout.log").read_text(), name)
        if read_json(directory / "process.json") != {"returncode": 0, "expected": 0}:
            raise WorldCheckError("Loader test process receipt did not succeed")
