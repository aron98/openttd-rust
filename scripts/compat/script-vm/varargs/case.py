# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Imported by the variadic producer.
"""Expected native terminal stage and payload for defined source programs."""

from dataclasses import dataclass
from typing import Literal


@dataclass(frozen=True, slots=True)
class Case:
    name: str
    source: str
    stage: Literal["return", "runtime_error", "compile_error"]
    outcome: bytes
    budget_extension: tuple[int, ...] = ()
