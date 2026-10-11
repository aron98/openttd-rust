"""Strict prototype/frame/output proof using the shared raw receipt postcondition."""

import os
import sys
from pathlib import Path

from .calls import compare_scenarios
from .closure_cases import cases


def compare(native: Path, rust: Path, output: Path) -> None:
    os.environ["OTTD_SCRIPT_CLOSURE_TRACE"] = "1"
    compare_scenarios(native, rust, output, cases(), b"prototype main ")


if __name__ == "__main__":
    compare(*(Path(argument).resolve() for argument in sys.argv[1:]))
