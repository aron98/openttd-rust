# SPDX-License-Identifier: GPL-2.0-only
"""Explicit byte-bound fixtures for undefined native ctype arguments, never parity."""

import subprocess
from pathlib import Path
from typing import Final

from scripts.script_vm_observation import compare_observation
from scripts.world_check_support import (
    ROOT,
    WorldCheckError,
    read_json,
    run,
    write_json,
)

POLICY_STAGE: Final = "undefined_native_input"
# sqlexer.cpp:251: decoded >255 reaches isdigit in Lex's default branch.
# Exact fixture bodies matter: comments, NUL suffixes and decoder failures stay strict.
UNDEFINED_INPUTS: Final = {
    b"return 1;/*x*/" + encoded + b"\n": codepoint
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


def observe_native(argv: list[str], case: Path, source: bytes) -> None:
    """Retain raw native results even if undefined behavior terminates the process."""
    directory = case / "native"
    if source not in UNDEFINED_INPUTS:
        _ = run(argv, directory)
        return
    directory.mkdir(parents=True, exist_ok=False)
    write_json(directory / "argv.json", [argument for argument in argv])
    result = subprocess.run(
        argv, capture_output=True, timeout=65, check=False, cwd=ROOT
    )
    _ = (directory / "stdout.log").write_bytes(result.stdout)
    _ = (directory / "stderr.log").write_bytes(result.stderr)
    write_json(
        directory / "process.json",
        {"returncode": result.returncode, "policy": POLICY_STAGE},
    )


def compare_case(source: bytes, case: Path, stage: str) -> None:
    """Compare defined inputs strictly; independently assert explicit policy rejection."""
    codepoint = UNDEFINED_INPUTS.get(source)
    rust = (case / "rust/stdout.log").read_text()
    diagnostic = (case / "rust/stderr.log").read_text()
    if codepoint is None:
        if stage == POLICY_STAGE or "UndefinedNativeCharacter" in diagnostic:
            raise WorldCheckError("Unclassified native undefined input")
        compare_observation((case / "native/stdout.log").read_text(), rust, stage)
        return
    if stage != POLICY_STAGE:
        raise WorldCheckError("Undefined input mislabeled as native parity")
    expected = f"UndefinedNativeCharacter {{ codepoint: {codepoint}, context: Token }} at byte 14\n"
    if rust != "compile_error\n" or diagnostic != expected:
        raise WorldCheckError("Missing exact Rust undefined-input rejection")
    if read_json(case / "rust/process.json") != {"returncode": 0, "expected": 0}:
        raise WorldCheckError("Rust policy process failed")
    match read_json(case / "native/process.json"):
        case {"returncode": int(), "policy": "undefined_native_input"}:
            return
        case _:
            raise WorldCheckError("Missing raw undefined native process observation")


def compare_files(source: Path, prefix: Path, statuses: tuple[int, int]) -> None:
    """Apply the same policy to the standalone shell driver's retained files."""
    codepoint = UNDEFINED_INPUTS.get(source.read_bytes())
    rust = Path(f"{prefix}.rust.stdout").read_text()
    diagnostic = Path(f"{prefix}.rust.stderr").read_text()
    if codepoint is None:
        if statuses != (0, 0) or "UndefinedNativeCharacter" in diagnostic:
            raise WorldCheckError("Defined-input process failed or was reclassified")
        native = Path(f"{prefix}.native.stdout").read_text()
        if not native or native != rust:
            raise WorldCheckError("Defined-input observation differs")
        _ = Path(f"{prefix}.diff").write_text("")
        return
    expected = f"UndefinedNativeCharacter {{ codepoint: {codepoint}, context: Token }} at byte 14\n"
    if statuses[1] != 0 or rust != "compile_error\n" or diagnostic != expected:
        raise WorldCheckError("Missing exact Rust undefined-input rejection")
    _ = Path(f"{prefix}.policy").write_text(
        f"{POLICY_STAGE}: Rust rejection; native result is not parity\n"
    )


if __name__ == "__main__":
    import sys

    compare_files(
        Path(sys.argv[1]), Path(sys.argv[2]), (int(sys.argv[3]), int(sys.argv[4]))
    )
