"""Scoped exact native comparison; interactive words remain observed inputs."""

from __future__ import annotations

import hashlib
from dataclasses import dataclass
from pathlib import Path
from typing import Final

from .value import (
    DECODE,
    EvidenceError,
    Json,
    array,
    field,
    integer,
    keys,
    require,
    same,
)

VIEWS: Final = ("world", "schema", "derived", "content", "runtime", "movement")
INTERACTIVE: Final = frozenset(
    (view, edge, *middle, "interactive_random", str(word))
    for view, middle in (("runtime", ()), ("movement", ("live",)))
    for edge in ("before_save", "after_save")
    for word in (0, 1)
)


@dataclass(frozen=True, slots=True)
class Difference:
    path: tuple[str, ...]
    left: Json
    right: Json


def differences(left: Json, right: Json, path: tuple[str, ...]) -> list[Difference]:
    if same(left, right):
        return []
    if isinstance(left, dict) and isinstance(right, dict) and set(left) == set(right):
        return [
            item
            for key in sorted(left)
            for item in differences(left[key], right[key], (*path, key))
        ]
    if isinstance(left, list) and isinstance(right, list) and len(left) == len(right):
        return [
            item
            for index, (a, b) in enumerate(zip(left, right, strict=True))
            for item in differences(a, b, (*path, str(index)))
        ]
    return [Difference(path, left, right)]


def words(value: Json) -> tuple[int, int]:
    values = array(value)
    require("two interactive input words required", condition=len(values) == 2)
    result = integer(values[0]), integer(values[1])
    require(
        "interactive word range",
        condition=all(0 <= word <= 0xFFFFFFFF for word in result),
    )
    return result


def checkpoint_words(directory: Path, label: str) -> tuple[int, int]:
    runtime = DECODE((directory / f"{label}.runtime.json").read_text())
    return words(field(field(runtime, "before_save"), "interactive_random"))


def validate_inputs(
    directory: Path, labels: tuple[str, ...], initial: tuple[int, int]
) -> int:
    """Require unchanged observed inputs at all eight paths and every live event."""
    for view in VIEWS:
        require(
            "missing/extra checkpoint views",
            condition={p.name for p in directory.glob(f"*.{view}.json")}
            == {f"{label}.{view}.json" for label in labels},
        )
    for label in labels:
        for view in ("runtime", "movement"):
            value = DECODE((directory / f"{label}.{view}.json").read_text())
            for edge in ("before_save", "after_save"):
                settled = field(value, edge)
                live = field(settled, "live") if view == "movement" else settled
                require(
                    "interactive checkpoint evolution refused",
                    condition=words(field(live, "interactive_random")) == initial,
                )
    count = 0
    with (directory / "events.jsonl").open() as stream:
        for line in stream:
            event = DECODE(line)
            require(
                "interactive event evolution refused",
                condition=words(field(field(event, "live"), "interactive_random"))
                == initial,
            )
            count += 1
    return count


@dataclass(frozen=True, slots=True)
class Comparison:
    pairs: int
    strict_full_equal: bool
    independent_input_differences: tuple[Difference, ...]


def compare_pairs(
    left: Path, right: Path, pairs: tuple[tuple[str, str], ...]
) -> Comparison:
    """Compare all six views without changing either source or masking any field."""
    require("comparison pairs missing", condition=bool(pairs))
    allowed: list[Difference] = []
    for left_label, right_label in pairs:
        for view in VIEWS:
            a = DECODE((left / f"{left_label}.{view}.json").read_text())
            b = DECODE((right / f"{right_label}.{view}.json").read_text())
            for difference in differences(a, b, (view,)):
                if difference.path not in INTERACTIVE:
                    raise EvidenceError(
                        f"non-input native difference {difference.path}"
                    )
                allowed.append(difference)
    return Comparison(len(pairs) * len(VIEWS), not allowed, tuple(allowed))


def compare_runs(
    left: Path, right: Path, left_calls: int, right_calls: int, offset: int = 0
) -> Comparison:
    """Admit only complete equal-horizon or exact second-half continuation scopes."""
    require("comparison call bounds", condition=0 <= right_calls <= left_calls <= 256)
    require(
        "comparison scope",
        condition=(offset == 0 and left_calls == right_calls)
        or (
            offset == right_calls and left_calls >= 2 * right_calls and right_calls > 0
        ),
    )
    for directory, calls in ((left, left_calls), (right, right_calls)):
        result = DECODE((directory / "results.json").read_text())
        require(
            "comparison result horizon",
            condition=integer(field(result, "calls")) == calls,
        )
        require(
            "comparison native outcome",
            condition=field(result, "outcome")
            in {"native_completed", "native_crossing_witness"},
        )
        labels = ("initial", *(f"tick_{i}" for i in range(1, calls + 1)), "final")
        receipt = DECODE((directory / "interactive-input.json").read_text())
        keys(receipt, {"scope", "observed_initial_words", "initial_runtime_sha256"})
        require(
            "interactive receipt scope",
            condition=field(receipt, "scope")
            == "observed-process-input-no-interactive-consumption",
        )
        require(
            "interactive receipt input hash",
            condition=field(receipt, "initial_runtime_sha256")
            == hashlib.sha256(
                (directory / "initial.runtime.json").read_bytes()
            ).hexdigest(),
        )
        initial = words(field(receipt, "observed_initial_words"))
        require(
            "interactive input receipt mismatch",
            condition=initial == checkpoint_words(directory, "initial"),
        )
        _ = validate_inputs(directory, labels, initial)
    pairs = (
        (f"tick_{offset}" if offset else "initial", "initial"),
        *((f"tick_{offset + i}", f"tick_{i}") for i in range(1, right_calls + 1)),
        (
            f"tick_{offset + right_calls}"
            if offset and offset + right_calls < left_calls
            else "final",
            "final",
        ),
    )
    return compare_pairs(left, right, pairs)
