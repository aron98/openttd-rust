# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 scripts/check-grf-currency-ci.py
from __future__ import annotations

import os
import shutil
import sys
import tempfile
from pathlib import Path

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
from scripts.currency_ci_bindings import bind_currency, compiler_inputs
from scripts.currency_ci_evidence import validate_currency
from scripts.currency_ci_probes import live_probes
from scripts.currency_ci_roster import SELECTORS
from scripts.depot_build_archive import package_raw
from scripts.gameplay_foundations import digest, log_name
from scripts.grf_control_evidence import sequence, text
from scripts.grf_control_run import ControlRun
from scripts.language_ci_compare import compare, mapping
from scripts.world_check_support import WorldCheckError, at, read_json, write_json


def main() -> None:
    forbidden = {
        "OTTD_GRF_CURRENCY_CASE",
        "OTTD_GRF_CONTROL_CASE",
        "OTTD_GRF_LANGUAGE_CASE",
        "OTTD_REPLAY_PATH",
        "OTTD_GRF_CONTROL_MANIFEST",
        "OTTD_GRF_CONTROL_OUTPUT",
        "OTTD_GRF_CURRENCY_OUTPUT",
        "OTTD_GRF_LANGUAGE_OUTPUT",
    }
    if forbidden.intersection(os.environ):
        raise WorldCheckError("Currency CI refuses subset/replay/observer environment")
    root = Path(__file__).resolve().parents[1]
    layout = read_json(root / "scripts/currency-ci-layout.json")
    sources(root, layout)
    if at(layout, ("verified_native_corpus",)) is not True:
        raise WorldCheckError("Currency layout awaits actual complete native corpus")
    parent = root / ".artifacts"
    parent.mkdir(exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="grf-currency-", dir=parent))
    print(f"Artifacts: {output}", flush=True)
    oracle = Path(
        os.environ.get(
            "OTTD_GRF_ORACLE", str(root / ".reference/snapshot-build/openttd")
        )
    ).resolve(strict=True)
    job = ControlRun(root, output, oracle)
    job.provenance()
    compiler_inputs(root, output)
    binary = build_lib(job)
    for group, selector in SELECTORS.items():
        run_exact(
            job,
            binary,
            Selection(selector, actual=os.environ.get("OTTD_CURRENCY_TEST_SELECTOR")),
            {
                "OTTD_GRF_CURRENCY_ORACLE": str(oracle),
                "OTTD_GRF_CURRENCY_ARTIFACTS": str(output / group),
                "OTTD_GRF_CURRENCY_GUARDS": str(output / group),
            },
        )
    for value in sequence(at(layout, ("focused",))):
        selector = text(value)
        run_exact(job, binary, Selection(selector, ignored=False), {})
        process(output / "logs" / log_name(selector), binary, selector, ignored=False)
    validate_currency(root, output, oracle, layout)
    bind_currency(root, output, oracle, layout)
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
    _ = (output / "summary.txt").write_text("PASS currency 0A complete fixed corpus\n")
    print("PASS currency owner CI admission", flush=True)


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
    raw = read_json(output / "api/all-owner-indices/native/control.json")
    for index, value in enumerate(sequence(at(raw, ("baseline_sources",)))):
        source = Path(text(value))
        retained = baseline / f"{index}.grf"
        _ = shutil.copy2(source, retained)
        compare(digest(source), digest(retained))


if __name__ == "__main__":
    main()
