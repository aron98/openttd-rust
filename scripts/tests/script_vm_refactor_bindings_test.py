"""Every extracted script helper is mandatory before any native admission."""

from dataclasses import replace
from pathlib import Path
from typing import Final

import pytest

from scripts.script_vm_inputs import source_names
from scripts.script_vm_provenance import load_spec, verify_sources
from scripts.world_check_support import ROOT, WorldCheckError

EXTRACTED_INPUTS: Final = (
    "crates/ottd-script/src/heap/ownership.rs",
    "crates/ottd-script/src/vm/registers.rs",
    "crates/ottd-script/src/vm/root_sessions/session.rs",
    "crates/ottd-script/src/vm/array_sessions/state.rs",
    "crates/ottd-script/src/vm/array_sessions/execution.rs",
    "crates/ottd-script/src/vm/array_sessions/commands.rs",
    "scripts/script_vm_receipts.py",
    "scripts/tests/script_vm_policy_producer_checks.py",
    "scripts/tests/script_vm_policy_assertions.py",
    "scripts/tests/test_script_vm_policy_assertions.py",
    "scripts/tests/script_vm_refactor_bindings_test.py",
    "scripts/grf-control-ruff.toml",
)


@pytest.mark.parametrize("name", EXTRACTED_INPUTS)
@pytest.mark.parametrize("mutation", ["omit", "tamper"])
def test_extracted_source_refuses_omission_and_tampering(
    tmp_path: Path, name: str, mutation: str
) -> None:
    spec = load_spec(ROOT / "scripts/script-vm-manifest.json")
    assert name in source_names(ROOT)
    assert name in dict(spec.sources)
    altered = replace(
        spec,
        sources=tuple(
            (path, "0" * 64 if mutation == "tamper" and path == name else sha)
            for path, sha in spec.sources
            if mutation != "omit" or path != name
        ),
    )
    expected = (
        "VM source closure omits inputs"
        if mutation == "omit"
        else f"VM source changed: {name}"
    )
    with pytest.raises(WorldCheckError, match=expected):
        verify_sources(ROOT, tmp_path, altered)
