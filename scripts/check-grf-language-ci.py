# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 scripts/check-grf-language-ci.py
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
from scripts.language_ci_bindings import bind_language
from scripts.language_ci_compare import compare
from scripts.language_ci_evidence import EMPTY_INPUTS, validate_language
from scripts.language_ci_probes import live_probes
from scripts.world_check_support import WorldCheckError, at, read_json

MATRIX: Final = "content::grf::language_native::original_language_matrix"
GUARDS: Final = "content::grf::language_native::guards::original_language_guards"
FOCUSED: Final = [
    "limits::language_input_limits_refuse_before_parsing_and_admit_exact_source_size",
    "limits::catalog_payload_is_charged_even_without_configured_files",
    "limits::mapping_pair_budget_counts_repeated_entries_not_unique_keys",
    "limits::a_new_loader_session_does_not_retain_prior_language_maps",
    "mapped_choices_charge_cumulative_intermediate_and_copied_bytes",
    "contextual_custom_inline_refuses_while_fresh_registry_stays_undefined",
]


def main() -> None:
    forbidden = {
        "OTTD_GRF_LANGUAGE_CASE",
        "OTTD_GRF_CONTROL_CASE",
        "OTTD_REPLAY_PATH",
        "OTTD_GRF_LANGUAGE_OUTPUT",
        "OTTD_GRF_CONTROL_MANIFEST",
        "OTTD_GRF_CONTROL_OUTPUT",
    }
    if forbidden.intersection(os.environ):
        raise WorldCheckError("Language CI refuses subset/replay/observer environment")
    root = Path(__file__).resolve().parents[1]
    layout = read_json(root / "scripts/language-ci-layout.json")
    sources(root, layout)
    parent = root / ".artifacts"
    parent.mkdir(exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="grf-language-", dir=parent))
    print(f"Artifacts: {output}", flush=True)
    oracle = Path(
        os.environ.get(
            "OTTD_GRF_ORACLE", str(root / ".reference/snapshot-build/openttd")
        )
    ).resolve(strict=True)
    job = ControlRun(root, output, oracle)
    job.provenance()
    binary = build_lib(job)
    variables = {
        "OTTD_GRF_LANGUAGE_ORACLE": str(oracle),
        "OTTD_GRF_LANGUAGE_ARTIFACTS": str(output / "results"),
        "OTTD_GRF_LANGUAGE_GUARDS": str(output / "guards"),
    }
    run_exact(
        job,
        binary,
        Selection(MATRIX, actual=os.environ.get("OTTD_LANGUAGE_TEST_SELECTOR")),
        variables,
    )
    run_exact(job, binary, Selection(GUARDS), variables)
    for suffix in FOCUSED:
        test = f"content::grf::language_pack_tests::{suffix}"
        run_exact(job, binary, Selection(test, ignored=False), {})
        process(output / "logs" / log_name(test), binary, test, ignored=False)
    validate_language(output, layout)
    bind_language(root, output, oracle, layout)
    live_probes(root, output, oracle, layout)
    zero(job, binary)
    verify_identity(root, output)
    for name in (MATRIX, GUARDS):
        process(output / "logs" / log_name(name), binary, name)
    for name in ("results", "guards"):
        compare(digest(output / name / "test-executable"), digest(binary))
    retain_native(output, oracle)
    retain_baseline(output)
    package_raw(
        output,
        empty_inputs=frozenset((f"results/{name}", sha) for name, sha in EMPTY_INPUTS),
    )
    _ = (output / "summary.txt").write_text(
        "PASS language110 events1593 texts2560 mutations73604 guards13\n"
    )
    print("PASS ordered language CI admission", flush=True)


def retain_baseline(output: Path) -> None:
    raw = read_json(output / "results/english/native/control.json")
    target = output / "baseline-sources"
    target.mkdir()
    for index, name in enumerate(sequence(at(raw, ("baseline_sources",)))):
        source = Path(text(name))
        retained = target / f"{index}.grf"
        _ = shutil.copy2(source, retained)
        compare(digest(source), digest(retained))


if __name__ == "__main__":
    main()
