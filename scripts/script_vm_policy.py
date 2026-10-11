# SPDX-License-Identifier: GPL-2.0-only
"""Explicit byte-bound fixtures for undefined native ctype arguments, never parity."""

from dataclasses import dataclass
from pathlib import Path
from typing import Final, Literal

from scripts.script_vm_observation import compare_observation
from scripts.world_check_support import (
    WorldCheckError,
    read_json,
    run,
    write_json,
)

POLICY_STAGE: Final = "undefined_native_input"


@dataclass(frozen=True, slots=True)
class UndefinedInput:
    """Exact reached byte-classifier diagnostic for one admitted fixture body."""

    codepoint: int
    context: Literal["Token", "HexEscape"]
    offset: int

    def diagnostic(self) -> str:
        return (
            f"UndefinedNativeCharacter {{ codepoint: {self.codepoint}, "
            f"context: {self.context} }} at byte {self.offset}\n"
        )


# sqlexer.cpp:251: decoded >255 reaches isdigit in Lex's default branch.
# Exact fixture bodies matter: comments, NUL suffixes and decoder failures stay strict.
UNDEFINED_INPUTS: Final = {
    b"return 1;/*x*/" + encoded + b"\n": UndefinedInput(codepoint, "Token", 14)
    for encoded, codepoint in (
        (b"\xdf\xbf", 0x7FF),
        (b"\xe0\xa0\x80", 0x800),
        (b"\xed\x9f\xbf", 0xD7FF),
        (b"\xed\xa0\x80", 0xD800),
        (b"\xed\xbf\xbf", 0xDFFF),
        (b"\xee\x80\x80", 0xE000),
        (b"\xef\xbb\xbf", 0xFEFF),
        (b"\xef\xbf\xbf", 0xFFFF),
    )
}
# ReadString 306/310: only these complete escaped-literal bodies are policy cases.
UNDEFINED_INPUTS.update(
    {
        prefix + encoded + b'";': UndefinedInput(codepoint, "HexEscape", len(prefix))
        for prefix in (b'return "\\x', b'return "\\x1', b'return "\\x1234')
        for encoded, codepoint in (
            (b"\xc4\x80", 0x100),
            (b"\xe2\x82\xac", 0x20AC),
            (b"\xed\xa0\x80", 0xD800),
        )
    }
)


def observe_native(argv: list[str], case: Path, source: bytes) -> None:
    """Execute defined inputs only; retain a refusal receipt for classified bytes."""
    if source not in UNDEFINED_INPUTS:
        _ = run(argv, case / "native")
        return
    case.mkdir(parents=True, exist_ok=False)
    write_json(case / "policy.json", {"native_executed": False, "reason": POLICY_STAGE})


def compare_case(source: bytes, case: Path, stage: str) -> None:
    """Compare defined inputs strictly and require exact policy rejection."""
    policy = UNDEFINED_INPUTS.get(source)
    rust = (case / "rust/stdout.log").read_text()
    diagnostic = (case / "rust/stderr.log").read_text()
    if policy is None:
        if stage == POLICY_STAGE or "UndefinedNativeCharacter" in diagnostic:
            raise WorldCheckError("Unclassified native undefined input")
        compare_observation((case / "native/stdout.log").read_text(), rust, stage)
        return
    if stage != POLICY_STAGE:
        raise WorldCheckError("Undefined input mislabeled as native parity")
    expected = policy.diagnostic()
    if rust != "compile_error\n" or diagnostic != expected:
        raise WorldCheckError("Missing exact Rust undefined-input rejection")
    if read_json(case / "rust/process.json") != {"returncode": 0, "expected": 0}:
        raise WorldCheckError("Rust policy process failed")
    if (case / "native").exists() or (case / "native").is_symlink():
        raise WorldCheckError("Undefined input has native execution evidence")
    receipt = case / "policy.json"
    if not receipt.is_file() or receipt.is_symlink():
        raise WorldCheckError("Missing undefined-input policy receipt")
    match read_json(receipt):
        case {
            "native_executed": False,
            "reason": "undefined_native_input",
        } as value if len(value) == 2:
            return
        case _:
            raise WorldCheckError("Invalid undefined-input policy receipt")


def observe_pair(
    source: Path, case: Path, native: str, rust: str, schedules: list[str]
) -> None:
    """Run the standalone driver's pair through the same defined-only boundary."""
    body = source.read_bytes()
    observe_native([native, str(source), *schedules], case, body)
    result = run([rust, str(source), *schedules], case / "rust")
    if body in UNDEFINED_INPUTS:
        compare_case(body, case, POLICY_STAGE)
        return
    if "UndefinedNativeCharacter" in result.stderr:
        raise WorldCheckError("Defined input was reclassified")
    output = (case / "native/stdout.log").read_text()
    if not output or output != result.stdout:
        raise WorldCheckError("Defined-input observation differs")


if __name__ == "__main__":
    import sys

    observe_pair(
        Path(sys.argv[1]), Path(sys.argv[2]), sys.argv[3], sys.argv[4], sys.argv[5:]
    )
