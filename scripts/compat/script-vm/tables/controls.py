"""Require real native capture corruptions to fail the production comparator."""

import json
import sys
from pathlib import Path

from .case import Case
from .compare import (
    BUDGETS,
    Observation,
    digest,
    validate,
    validate_runtime_stderr,
    verify_observation,
)


def controls(output: Path) -> None:
    native = (output / "empty-1-native.stdout").read_bytes()
    source = (output / "empty.nut").read_text()
    case = Case("empty", source)
    variants = {
        "opcode": native.replace(b"op 30 ", b"op 31 ", 1),
        "result": native.replace(b"7461626c65", b"6172726179", 1),
        "debt": native.replace(b"suspend -1 ", b"suspend -2 ", 1),
        "cursor": native.replace(b"suspend -1 0", b"suspend -1 1", 1),
        "vacuous-compilation": b"compile_error\n",
    }
    rejected: list[str] = []
    for name, changed in variants.items():
        if changed == native:
            raise RuntimeError(f"Control did not change its input: {name}")
        _ = (output / f"control-{name}.stdout").write_bytes(changed)
        left = changed if name == "vacuous-compilation" else native
        try:
            validate(case, left, changed)
        except RuntimeError as error:
            _ = (output / f"control-{name}.error").write_text(str(error))
            rejected.append(name)
        else:
            raise RuntimeError(f"Comparator accepted corruption: {name}")
    runtime = (output / "missing-get-0-native.stderr").read_bytes()
    if not runtime:
        raise RuntimeError("Missing genuine native runtime diagnostic")
    changed_error = runtime + b"corrupted runtime diagnostic\n"
    _ = (output / "control-runtime-stderr.stderr").write_bytes(changed_error)
    try:
        validate_runtime_stderr(
            Case("missing-get", "", "runtime_error"), runtime, changed_error
        )
    except RuntimeError as error:
        _ = (output / "control-runtime-stderr.error").write_text(str(error))
        rejected.append("runtime-stderr")
    else:
        raise RuntimeError("Comparator accepted runtime stderr corruption")
    compiler_control(output)
    rejected.append("compiler-stderr")
    _ = (output / "controls.json").write_text(json.dumps(rejected) + "\n")
    print(f"PASS rejected {len(rejected)} native observation corruptions")


def compiler_control(output: Path) -> None:
    """Reject a rehashed mutation of genuine retained compiler stderr."""
    output = output.resolve()
    case = Case("invalid-literal", "return {a:1};", "compile_error")
    source = output / "invalid-literal.nut"
    prefix = "invalid-literal-0-rust"
    stderr = output / f"{prefix}.stderr"
    receipt = output / f"{prefix}.json"
    original_error, original_receipt = stderr.read_bytes(), receipt.read_bytes()
    if not original_error:
        raise RuntimeError("Missing real compiler error")
    record = Observation(
        case.name,
        0,
        BUDGETS[0],
        source.name,
        "invalid-literal-0-native",
        prefix,
        digest(source),
        digest(output / f"{prefix}.stdout"),
    )
    verify_observation(output, record, (case, BUDGETS[0]))
    try:
        _ = stderr.write_bytes(original_error + b"compiler corruption\n")
        _ = receipt.write_text(
            json.dumps(
                {
                    "argv": [str(output / "rust"), str(source), *map(str, BUDGETS[0])],
                    "status": 0,
                    "stdout_sha256": digest(output / f"{prefix}.stdout"),
                    "stderr_sha256": digest(stderr),
                }
            )
        )
        _ = (output / "control-compiler-stderr.stderr").write_bytes(stderr.read_bytes())
        _ = (output / "control-compiler-stderr.json").write_bytes(receipt.read_bytes())
        try:
            verify_observation(output, record, (case, BUDGETS[0]))
        except RuntimeError as error:
            _ = (output / "control-compiler-stderr.error").write_text(str(error))
        else:
            raise RuntimeError("Comparator accepted compiler stderr corruption")
    finally:
        _ = stderr.write_bytes(original_error)
        _ = receipt.write_bytes(original_receipt)
    verify_observation(output, record, (case, BUDGETS[0]))


if __name__ == "__main__":
    controls(Path(sys.argv[1]))
