"""Strict expected stage and deterministic length-delimited source literals."""

from dataclasses import dataclass
from typing import Literal


@dataclass(frozen=True, slots=True)
class Case:
    name: str
    source: str
    stage: Literal["return", "runtime_error", "compile_error"] = "return"
    error_trace: bool = False


def literal(length: int, seed: int) -> str:
    state = seed + 1
    data: list[str] = []
    for _ in range(length):
        state ^= (state << 13) & ((1 << 64) - 1)
        state ^= state >> 7
        state ^= (state << 17) & ((1 << 64) - 1)
        data.append(f"\\x{state & 255:02x}")
    return '"' + "".join(data) + '"'
