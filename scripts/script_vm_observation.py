# SPDX-License-Identifier: GPL-2.0-only
"""Exact executable test membership and native observation comparison."""

import re

from scripts.world_check_support import WorldCheckError


def require_tests(output: str, names: tuple[str, ...] | list[str]) -> None:
    actual = re.findall(r"^test (\S+) \.\.\. ok$", output, re.MULTILINE)
    summaries = re.findall(
        r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;",
        output,
        re.MULTILINE,
    )
    if sorted(actual) != sorted(names) or summaries != [(str(len(names)), "0", "0")]:
        raise WorldCheckError("VM tests did not execute exact membership")


def compare_observation(native: str, rust: str, stage: str) -> None:
    if not native.strip() or native != rust:
        raise WorldCheckError("VM observation differs or is empty")
    lines = native.splitlines()
    if lines[-1].split()[0] != stage:
        raise WorldCheckError("VM outcome stage differs")
    if stage == "compile_error":
        if lines != ["compile_error"]:
            raise WorldCheckError("Invalid compile failure observation")
    elif not lines[0].startswith("stack ") or not any(
        line.startswith("op ") for line in lines
    ):
        raise WorldCheckError("Missing compiled scalar instructions")
