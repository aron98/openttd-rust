# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 scripts/check-grf-strings-ci.py
from __future__ import annotations

import os
import shutil
import sys
import tempfile
from pathlib import Path
from typing import Final

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.context_ci_support import (
    Selection,
    build_lib,
    process,
    retain_native,
    run_exact,
    sources,
    verify_identity,
    zero,
)
from scripts.depot_build_archive import package_raw
from scripts.gameplay_foundations import digest, log_name
from scripts.grf_control_evidence import sequence, text
from scripts.grf_control_run import ControlRun
from scripts.language_ci_compare import compare, mapping
from scripts.strings_ci_bindings import bind_strings
from scripts.strings_ci_evidence import validate_strings
from scripts.strings_ci_probes import live_probes
from scripts.world_check_support import WorldCheckError, at, read_json, write_json

SELECTORS: Final = {
    "api": "content::grf::load_strings_native::original_string_api_matrix",
    "load": "content::grf::load_strings_load_native::original_string_load_matrix",
    "guards": "content::grf::load_strings_guards::original_string_guards",
}


def main() -> None:
    forbidden = {
        "OTTD_GRF_STRINGS_CASE",
        "OTTD_GRF_CONTROL_CASE",
        "OTTD_GRF_LANGUAGE_CASE",
        "OTTD_REPLAY_PATH",
        "OTTD_GRF_CONTROL_MANIFEST",
        "OTTD_GRF_CONTROL_OUTPUT",
        "OTTD_GRF_STRINGS_OUTPUT",
        "OTTD_GRF_LANGUAGE_OUTPUT",
    }
    if forbidden.intersection(os.environ):
        raise WorldCheckError("String CI refuses subset/replay/observer environment")
    root = Path(__file__).resolve().parents[1]
    layout = read_json(root / "scripts/strings-ci-layout.json")
    sources(root, layout)
    parent = root / ".artifacts"
    parent.mkdir(exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="grf-strings-", dir=parent))
    print(f"Artifacts: {output}", flush=True)
    oracle = Path(
        os.environ.get(
            "OTTD_GRF_ORACLE", str(root / ".reference/snapshot-build/openttd")
        )
    ).resolve(strict=True)
    job = ControlRun(root, output, oracle)
    job.provenance()
    binary = build_lib(job)
    for group, selector in SELECTORS.items():
        variables = {
            "OTTD_GRF_STRINGS_ORACLE": str(oracle),
            "OTTD_GRF_STRINGS_ARTIFACTS": str(output / group),
            "OTTD_GRF_STRINGS_GUARDS": str(output / group),
        }
        run_exact(
            job,
            binary,
            Selection(selector, actual=os.environ.get("OTTD_STRINGS_TEST_SELECTOR")),
            variables,
        )
    for name in sequence(at(layout, ("focused",))):
        run_exact(job, binary, Selection(text(name), ignored=False), {})
        process(
            output / "logs" / log_name(text(name)), binary, text(name), ignored=False
        )
    validate_strings(output, layout)
    bind_strings(root, output, oracle, layout)
    live_probes(root, output, oracle, layout)
    zero(job, binary)
    verify_identity(root, output)
    for group, selector in SELECTORS.items():
        process(output / "logs" / log_name(selector), binary, selector)
        compare(digest(output / group / "test-executable"), digest(binary))
    retain_native(output, oracle)
    retain_sources(root, output)
    write_json(output / "coverage.json", at(layout, ("totals",)))
    package_raw(output)
    _ = (output / "summary.txt").write_text(
        "PASS strings API10/1691/11198 loader90/14659/91117 guards7\n"
    )
    print("PASS session string CI admission", flush=True)


def retain_sources(root: Path, output: Path) -> None:
    for name, sha in mapping(
        at(read_json(output / "provenance.json"), ("source_hashes",))
    ).items():
        destination = output / "source" / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        _ = shutil.copy2(root / name, destination)
        compare(digest(destination), sha)
    baseline = output / "baseline-sources"
    baseline.mkdir()
    raw = read_json(output / "api/old-language-masks/native/control.json")
    for index, value in enumerate(sequence(at(raw, ("baseline_sources",)))):
        source = Path(text(value))
        retained = baseline / f"{index}.grf"
        _ = shutil.copy2(source, retained)
        compare(digest(source), digest(retained))


if __name__ == "__main__":
    main()
