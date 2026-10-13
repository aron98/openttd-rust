"""Execute and compare every declared CLI/runtime and encoded-continuation branch."""

from __future__ import annotations

from dataclasses import dataclass
from enum import StrEnum
from pathlib import Path
from typing import assert_never

from .compare import INTERACTIVE, compare_runs
from .native import Native, write
from .physical import compare as compare_physical
from .protocol import labels, loaded
from .qualify import Qualified
from .rust import Rust
from .value import DECODE, require, same


class Surface(StrEnum):
    CLI = "public-cli-saved-state"
    RUNTIME = "public-runtime-saved-physical-single-bucket"


@dataclass(frozen=True, slots=True)
class ViewScope:
    calls: int
    subject: int
    surface: Surface
    offset: int = 0


def rust_views(native: Path, rust: Path, scope: ViewScope) -> None:
    calls, subject, surface, offset = (
        scope.calls,
        scope.subject,
        scope.surface,
        scope.offset,
    )
    expected = labels(calls)
    for suffix in ("world", "derived"):
        require(
            "Rust complete checkpoint roster",
            condition={p.name for p in rust.glob(f"*.{suffix}.json")}
            == {f"{label}.{suffix}.json" for label in expected},
        )
    match surface:
        case Surface.RUNTIME:
            require(
                "Rust physical checkpoint roster",
                condition={p.name for p in rust.glob("*.physical.json")}
                == {f"{label}.physical.json" for label in expected},
            )
        case Surface.CLI:
            require(
                "CLI cannot claim physical cache view",
                condition=not list(rust.glob("*.physical.json")),
            )
        case _:
            assert_never(surface)
    for label in expected:
        native_label = label
        if label == "initial":
            native_label = f"tick_{offset}" if offset else "initial"
        elif label == "final":
            native_label = f"tick_{offset + calls}" if offset + calls else "initial"
        else:
            native_label = f"tick_{offset + int(label[5:])}"
        for suffix in ("world", "derived"):
            actual = DECODE((rust / f"{label}.{suffix}.json").read_text())
            original = DECODE((native / f"{native_label}.{suffix}.json").read_text())
            require(
                f"complete Rust {suffix} mismatch at {label}",
                condition=same(actual, original),
            )
        match surface:
            case Surface.RUNTIME:
                require(
                    "complete Rust content mismatch",
                    condition=same(
                        DECODE((rust / f"{label}.content.json").read_text()),
                        DECODE((native / f"{native_label}.content.json").read_text()),
                    ),
                )
                compare_physical(
                    DECODE((native / f"{native_label}.movement.json").read_text()),
                    DECODE((rust / f"{label}.physical.json").read_text()),
                    subject,
                )
            case Surface.CLI:
                require(
                    "CLI saved checkpoint absent",
                    condition=(rust / f"{label}.sav").is_file(),
                )
            case _:
                assert_never(surface)


def run(native: Native, rust: Rust, case: Qualified) -> None:
    stem = case.case.name.replace("-", "_")
    q, h = (case.horizon, case.crossing)
    cli_zero = rust.replay(stem + "_cli_zero", case.save, 0)
    rust_views(case.traced, cli_zero, ViewScope(0, int(case.subject), Surface.CLI))
    runtime_zero = rust.capture(stem + "_runtime_zero", case.save, 0)
    rust_views(
        case.traced, runtime_zero, ViewScope(0, int(case.subject), Surface.RUNTIME)
    )
    cli = rust.replay(stem + "_cli", case.save, q)
    rust_views(case.traced, cli, ViewScope(q, int(case.subject), Surface.CLI))
    runtime = rust.capture(stem + "_runtime", case.save, q)
    rust_views(case.traced, runtime, ViewScope(q, int(case.subject), Surface.RUNTIME))
    cli_crossing = rust.replay(stem + "_cli_crossing", case.save, h)
    rust_views(case.traced, cli_crossing, ViewScope(h, int(case.subject), Surface.CLI))
    runtime_crossing = rust.capture(stem + "_runtime_crossing", case.save, h)
    rust_views(
        case.traced, runtime_crossing, ViewScope(h, int(case.subject), Surface.RUNTIME)
    )
    original_h = case.traced / f"tick_{h}.sav"
    rust_h = runtime / f"tick_{h}.sav"
    cli_from_original = rust.replay(stem + "_cli_from_original", original_h, h)
    rust_views(
        case.traced, cli_from_original, ViewScope(h, int(case.subject), Surface.CLI, h)
    )
    runtime_from_original = rust.capture(stem + "_runtime_from_original", original_h, h)
    rust_views(
        case.traced,
        runtime_from_original,
        ViewScope(h, int(case.subject), Surface.RUNTIME, h),
    )
    runtime_from_rust = rust.capture(stem + "_runtime_from_rust", rust_h, h)
    rust_views(
        case.traced,
        runtime_from_rust,
        ViewScope(h, int(case.subject), Surface.RUNTIME, h),
    )
    cli_from_rust = rust.replay(stem + "_cli_from_rust", rust_h, h)
    rust_views(
        case.traced, cli_from_rust, ViewScope(h, int(case.subject), Surface.CLI, h)
    )
    original_from_rust = native.run(
        stem + "_original_from_rust",
        cli / f"tick_{h}.sav",
        loaded("replay", case.subject, h, trace=True),
        "native_completed",
    )
    comparison = compare_runs(case.traced, original_from_rust, q, h, h)
    for label in labels(h):
        position = h
        if label == "final":
            position += h
        elif label != "initial":
            position += int(label[5:])
        compare_physical(
            DECODE((original_from_rust / f"{label}.movement.json").read_text()),
            DECODE((runtime / f"tick_{position}.physical.json").read_text()),
            int(case.subject),
        )
    write(
        native.output / (stem + "_matrix.json"),
        {
            "case": case.case.name,
            "qualification": q,
            "crossing": h,
            "original_compared_views": comparison.pairs,
            "strict_original_full_runtime_equal": comparison.strict_full_equal,
            "rust_capability": Surface.RUNTIME.value,
            "rust_viewport": False,
            "original_input_origin": "public-cli-tickH",
            "new_original_replay_from_rust": True,
            "independent_paths": [list(path) for path in sorted(INTERACTIVE)],
            "native_from_rust_independent_input_differences": [
                {"path": list(row.path), "left": row.left, "right": row.right}
                for row in comparison.independent_input_differences
            ],
            "surfaces": [
                str(path)
                for path in (
                    cli_zero,
                    runtime_zero,
                    cli_crossing,
                    runtime_crossing,
                    cli,
                    runtime,
                    cli_from_original,
                    runtime_from_original,
                    runtime_from_rust,
                    cli_from_rust,
                    original_from_rust,
                )
            ],
        },
    )
