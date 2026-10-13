"""Typed JSON boundary primitives with exact scalar identity."""

from __future__ import annotations

import json
from collections.abc import Callable
from typing import TypeAlias

Json: TypeAlias = dict[str, "Json"] | list["Json"] | str | int | float | bool | None
DECODE: Callable[[str], Json] = json.loads


class EvidenceError(ValueError):
    """An evidence value fails the declared contract."""


def require(message: str, *, condition: bool) -> None:
    if not condition:
        raise EvidenceError(message)


def field(value: Json, name: str) -> Json:
    require(f"object required for {name}", condition=isinstance(value, dict))
    if not isinstance(value, dict) or name not in value:
        raise EvidenceError(f"missing field {name}")
    return value[name]


def keys(value: Json, expected: set[str]) -> None:
    if not isinstance(value, dict) or set(value) != expected:
        raise EvidenceError("missing or extra object fields")


def integer(value: Json) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise EvidenceError("integer required")
    return value


def string(value: Json) -> str:
    if not isinstance(value, str):
        raise EvidenceError("string required")
    return value


def array(value: Json) -> list[Json]:
    if not isinstance(value, list):
        raise EvidenceError("array required")
    return value


def same(left: Json, right: Json) -> bool:
    return json.dumps(left, sort_keys=True, allow_nan=False) == json.dumps(
        right, sort_keys=True, allow_nan=False
    )
