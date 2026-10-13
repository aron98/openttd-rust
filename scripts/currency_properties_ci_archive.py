from __future__ import annotations

import shutil
from pathlib import Path

from scripts.context_ci_support import process, retain_native, verify_identity
from scripts.currency_properties_ci_roster import API, SELECTORS
from scripts.gameplay_foundations import digest, log_name
from scripts.grf_control_evidence import sequence, text
from scripts.grf_control_run import ControlRun
from scripts.language_ci_compare import compare, mapping
from scripts.world_check_support import at, read_json


def retain(job: ControlRun, binary: Path) -> None:
    verify_identity(job.root, job.output)
    for group, selector in SELECTORS.items():
        process(job.output / "logs" / log_name(selector), binary, selector)
        compare(digest(job.output / group / "test-executable"), digest(binary))
    retain_native(job.output, job.oracle)
    for name, sha in mapping(
        at(read_json(job.output / "provenance.json"), ("source_hashes",))
    ).items():
        destination = job.output / "source" / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        _ = shutil.copy2(job.root / name, destination)
        compare(digest(destination), sha)
    baseline = job.output / "baseline-sources"
    baseline.mkdir()
    raw = read_json(job.output / "api" / API[0] / "native/control.json")
    for index, value in enumerate(sequence(at(raw, ("baseline_sources",)))):
        source = Path(text(value))
        retained = baseline / f"{index}.grf"
        _ = shutil.copy2(source, retained)
        compare(digest(source), digest(retained))
