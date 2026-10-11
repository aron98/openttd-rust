"""Corrupt genuine error objects, trace admission and rehashed diagnostic receipts."""

import json
import sys
from dataclasses import asdict
from pathlib import Path

from .case import Case
from .cases import cases
from .compare import (
    BUDGETS,
    Invocation,
    Observation,
    digest,
    source_arguments,
    validate,
    verify_observation,
)


def record(output: Path, case: Case) -> Observation:
    prefix = f"{case.name}-0"
    return Observation(
        case.name,
        0,
        BUDGETS[0],
        f"{case.name}.nut",
        f"{prefix}-native",
        f"{prefix}-rust",
        digest(output / f"{case.name}.nut"),
        digest(output / f"{prefix}-native.stdout"),
    )


def reject_output(
    output: Path, case: Case, name: str, left: bytes, right: bytes
) -> None:
    _ = (output / f"trap-control-{name}.stdout").write_bytes(right)
    try:
        validate(case, left, right)
    except RuntimeError as error:
        _ = (output / f"trap-control-{name}.error").write_text(str(error))
    else:
        raise RuntimeError(f"Comparator admitted error-object corruption: {name}")


def reject_receipt(
    output: Path, case: Case, name: str, *, omit_trace: bool = False
) -> None:
    observed = record(output, case)
    verify_observation(output, observed, (case, BUDGETS[0]))
    stdout = output / f"{case.name}-0-rust.stdout"
    stderr = output / f"{case.name}-0-rust.stderr"
    receipt = output / f"{case.name}-0-rust.json"
    saved_error, saved_receipt = stderr.read_bytes(), receipt.read_bytes()
    try:
        args = source_arguments(case, output / f"{case.name}.nut", BUDGETS[0])
        if omit_trace:
            args.remove("--errors")
        else:
            if not saved_error:
                raise RuntimeError("Control requires genuine nonempty native stderr")
            _ = stderr.write_bytes(saved_error + b"corrupted diagnostic\n")
        invocation = Invocation(
            [str(output / "rust"), *args], 0, digest(stdout), digest(stderr)
        )
        _ = receipt.write_text(json.dumps(asdict(invocation), indent=2) + "\n")
        _ = (output / f"trap-control-{name}.json").write_bytes(receipt.read_bytes())
        _ = (output / f"trap-control-{name}.stderr").write_bytes(stderr.read_bytes())
        try:
            verify_observation(output, observed, (case, BUDGETS[0]))
        except RuntimeError as error:
            _ = (output / f"trap-control-{name}.error").write_text(str(error))
        else:
            raise RuntimeError(f"Comparator admitted receipt corruption: {name}")
    finally:
        _ = stderr.write_bytes(saved_error)
        _ = receipt.write_bytes(saved_receipt)
    verify_observation(output, observed, (case, BUDGETS[0]))


def controls(output: Path) -> None:
    output = output.resolve()
    roster = {case.name: case for case in cases()}
    case = roster["trap-uncaught_integer"]
    observed = record(output, case)
    verify_observation(output, observed, (case, BUDGETS[0]))
    original = (output / f"{case.name}-0-native.stdout").read_bytes()
    snapshot = b"last_error integer 42\n"
    if original.count(snapshot) != 1:
        raise RuntimeError("Missing genuine integer throw snapshot")
    missing = original.replace(snapshot, b"")
    mutations = {
        "missing-channel": missing,
        "duplicate-channel": original + snapshot,
        "misplaced-channel": missing + snapshot,
        "malformed-object": original.replace(
            snapshot, b"last_error integer forty-two\n"
        ),
    }
    for name, changed in mutations.items():
        if changed == original:
            raise RuntimeError("Control failed to mutate its actual input")
        reject_output(output, case, name, changed, changed)
    reject_output(
        output,
        case,
        "changed-object",
        original,
        original.replace(snapshot, b"last_error integer 43\n"),
    )
    reject_receipt(output, case, "missing-trace-option", omit_trace=True)
    reject_receipt(output, roster["trap-uncaught_string"], "runtime-stderr")
    reject_receipt(output, roster["trap-compiler-missing-catch"], "compiler-stderr")
    verify_observation(output, observed, (case, BUDGETS[0]))
    rejected = [
        *mutations,
        "changed-object",
        "missing-trace-option",
        "runtime-stderr",
        "compiler-stderr",
    ]
    _ = (output / "trap-controls.json").write_text(json.dumps(rejected) + "\n")
    print(f"PASS {len(rejected)} trap controls; restored baseline")


if __name__ == "__main__":
    controls(Path(sys.argv[1]))
