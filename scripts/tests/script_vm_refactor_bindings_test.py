"""Every extracted script helper is mandatory before any native admission."""

from dataclasses import replace
from pathlib import Path
from typing import Final

import pytest

from scripts.script_vm_inputs import source_names
from scripts.script_vm_provenance import load_spec, verify_sources
from scripts.world_check_support import ROOT, WorldCheckError

EXTRACTED_INPUTS: Final = (
    "crates/ottd-script/src/runner/error_state.rs",
    "crates/ottd-script/src/vm/operations.rs",
    "crates/ottd-script/src/compiler/statements/traps.rs",
    "crates/ottd-script/src/vm/traps.rs",
    "crates/ottd-script/src/vm/traps/owners.rs",
    "crates/ottd-script/src/vm/traps/caught_owner.txt",
    "crates/ottd-script/src/vm/traps/caught_cycle_owner.txt",
    "crates/ottd-script/src/vm/traps/uncaught_array.txt",
    "crates/ottd-script/tests/runtime_diagnostics.rs",
    "crates/ottd-script/tests/traps.rs",
    "crates/ottd-script/examples/observe/errors.rs",
    "scripts/compat/script-vm/tables/trap_control_cases.py",
    "scripts/compat/script-vm/tables/trap_scope_cases.py",
    "scripts/compat/script-vm/tables/throw_cases.py",
    "scripts/compat/script-vm/tables/trap_boundary_cases.py",
    "scripts/compat/script-vm/tables/trap_compile_cases.py",
    "scripts/compat/script-vm/tables/trap_controls.py",
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
    "crates/ottd-script/src/compiler/expressions/delegates.rs",
    "crates/ottd-script/src/heap/collection.rs",
    "crates/ottd-script/src/heap/delegate_lifetimes.rs",
    "crates/ottd-script/src/heap/delegate_tests.rs",
    "crates/ottd-script/src/heap/delegates.rs",
    "crates/ottd-script/src/heap/owner_probe.rs",
    "crates/ottd-script/src/heap/release_tests.rs",
    "crates/ottd-script/src/table/data.rs",
    "crates/ottd-script/src/table/delegates.rs",
    "crates/ottd-script/src/vm/delegates.rs",
    "crates/ottd-script/src/vm/membership.rs",
    "crates/ottd-script/src/vm/membership_tests.rs",
    "crates/ottd-script/tests/delegate-lifetimes/capture.nut",
    "crates/ottd-script/tests/delegate-lifetimes/capture.txt",
    "crates/ottd-script/tests/delegate-lifetimes/default.nut",
    "crates/ottd-script/tests/delegate-lifetimes/default.txt",
    "crates/ottd-script/tests/delegate-lifetimes/error.nut",
    "crates/ottd-script/tests/delegate-lifetimes/error.txt",
    "crates/ottd-script/tests/delegate-owners/gc-own.txt",
    "crates/ottd-script/tests/delegate-owners/gc.txt",
    "crates/ottd-script/tests/delegate-owners/marked.txt",
    "crates/ottd-script/tests/delegate-owners/mutations.txt",
    "crates/ottd-script/tests/delegate-owners/ordinary.txt",
    "crates/ottd-script/tests/delegate_policy.rs",
    "crates/ottd-script/tests/delegate_storage.rs",
    "crates/ottd-script/tests/delegation.rs",
    "crates/ottd-script/tests/delegation/attach-own-and-chain.nut",
    "crates/ottd-script/tests/delegation/builtin-type-dead-typeof.nut",
    "crates/ottd-script/tests/delegation/chain-three.nut",
    "crates/ottd-script/tests/delegation/clone-retains-delegate.nut",
    "crates/ottd-script/tests/delegation/closure-retained-delegate.nut",
    "crates/ottd-script/tests/delegation/cycle-direct-atomic.nut",
    "crates/ottd-script/tests/delegation/cycle-indirect-atomic.nut",
    "crates/ottd-script/tests/delegation/delegate-evaluation-order.nut",
    "crates/ottd-script/tests/delegation/delegate-type-atomic.nut",
    "crates/ottd-script/tests/delegation/delegated-method-this.nut",
    "crates/ottd-script/tests/delegation/delete-inherited-refuses.nut",
    "crates/ottd-script/tests/delegation/delete-own-reveals-parent.nut",
    "crates/ottd-script/tests/delegation/detach-alias.nut",
    "crates/ottd-script/tests/delegation/foreach-dead-nexti.nut",
    "crates/ottd-script/tests/delegation/foreach-own-only.nut",
    "crates/ottd-script/tests/delegation/immediate-meta-only.nut",
    "crates/ottd-script/tests/delegation/in-traverses-delegate.nut",
    "crates/ottd-script/tests/delegation/newslot-own-shadow.nut",
    "crates/ottd-script/tests/delegation/ordinary-slot-cycle.nut",
    "crates/ottd-script/tests/delegation/own-get-dead-hook.nut",
    "crates/ottd-script/tests/delegation/own-newslot-dead-hook.nut",
    "crates/ottd-script/tests/delegation/own-shadows-parent.nut",
    "crates/ottd-script/tests/delegation/parent-bare-root.nut",
    "crates/ottd-script/tests/delegation/parent-nontable-error.nut",
    "crates/ottd-script/tests/delegation/raw-in-skips-get-hook.nut",
    "crates/ottd-script/tests/delegation/receiver-error-before-delegate-type.nut",
    "crates/ottd-script/tests/delegation/root-delegate-read-write.nut",
    "crates/ottd-script/tests/delegation/same-identity-dead-cmp.nut",
    "crates/ottd-script/tests/delegation/set-existing-parent.nut",
    "crates/ottd-script/tests/delegation/set-nearest-parent.nut",
    "crates/ottd-script/tests/delegation/unicode-nul-key.nut",
    "crates/ottd-script/tests/membership.rs",
    "crates/ottd-script/tests/membership/array-numeric.nut",
    "crates/ottd-script/tests/membership/bitwise-precedence.nut",
    "crates/ottd-script/tests/membership/left-associative-chain.nut",
    "crates/ottd-script/tests/membership/no-default-method.nut",
    "crates/ottd-script/tests/membership/own-and-missing.nut",
    "crates/ottd-script/tests/membership/own-null-present.nut",
    "crates/ottd-script/tests/membership/scalar-missing.nut",
    "crates/ottd-script/tests/membership/short-circuit.nut",
    "crates/ottd-script/tests/membership/string-negative.nut",
    "crates/ottd-script/tests/required-metahooks/arithmetic-hook.nut",
    "crates/ottd-script/tests/required-metahooks/call-hook.nut",
    "crates/ottd-script/tests/required-metahooks/clone-throw-ignored.nut",
    "crates/ottd-script/tests/required-metahooks/comparison-hook.nut",
    "crates/ottd-script/tests/required-metahooks/default-table-len.nut",
    "crates/ottd-script/tests/required-metahooks/default-table-rawget.nut",
    "crates/ottd-script/tests/required-metahooks/default-table-rawin.nut",
    "crates/ottd-script/tests/required-metahooks/default-table-rawset.nut",
    "crates/ottd-script/tests/required-metahooks/delete-throw-falls-back.nut",
    "crates/ottd-script/tests/required-metahooks/get-failure-index-replacement.nut",
    "crates/ottd-script/tests/required-metahooks/get-hook-owns-receiver.nut",
    "crates/ottd-script/tests/required-metahooks/get-hook-success.nut",
    "crates/ottd-script/tests/required-metahooks/hook-budget-debt.nut",
    "crates/ottd-script/tests/required-metahooks/hook-catch-local.nut",
    "crates/ottd-script/tests/required-metahooks/newslot-throw-falls-back.nut",
    "crates/ottd-script/tests/required-metahooks/set-hook-success.nut",
    "crates/ottd-script/tests/required-metahooks/string-hook.nut",
    "crates/ottd-script/tests/required-metahooks/type-hook.nut",
    "scripts/compat/script-vm/delegates.hpp",
    "scripts/compat/script-vm/owner_trace.hpp",
    "scripts/compat/script-vm/tables/delegate_cases.py",
    "scripts/compat/script-vm/tables/delegate_owners.py",
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
