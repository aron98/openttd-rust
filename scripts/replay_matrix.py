# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 -m scripts.check-replay --artifacts FRESH_PATH
"""Public CLI and original-engine replay comparisons."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from scripts.world_check_support import (
    ROOT,
    Json,
    WorldCheckError,
    read_json,
    run,
    write_json,
)

FIXTURES = ROOT / "fixtures/replay"
HOST_FIELDS = frozenset({"interactive_random", "current_company", "native_metadata"})


def deterministic(value: Json) -> Json:
    """Remove only documented native host observers, never saved state fields."""
    match value:
        case dict() as fields:
            return {
                key: deterministic(item)
                for key, item in fields.items()
                if key not in HOST_FIELDS
            }
        case list() as items:
            return [deterministic(item) for item in items]
        case scalar:
            return scalar


def checkpoint_labels(results: Json) -> list[str]:
    match results:
        case {"schema_version": 1, "checkpoints": list() as checkpoints}:
            labels: list[str] = []
            for checkpoint in checkpoints:
                match checkpoint:
                    case {"label": str() as label}:
                        labels.append(label)
                    case _:
                        raise WorldCheckError("Invalid replay checkpoint record")
            if (
                len(labels) != len(set(labels))
                or labels[0:1] != ["initial"]
                or labels[-1:] != ["final"]
            ):
                raise WorldCheckError("Incomplete/duplicate replay checkpoints")
            return labels
        case _:
            raise WorldCheckError("Invalid replay results schema")


@dataclass(frozen=True, slots=True)
class ReplayMatrix:
    cli: Path
    oracle: Path
    original: Path
    artifacts: Path

    def compare(self, expected: Path, actual: Path, log: Path) -> None:
        _ = run([str(self.cli), "compare", str(expected), str(actual)], log)

    def native(self, save: Path, actions: Path, case: Path) -> Path:
        output = case / "native"
        _ = run(
            [
                "cmake",
                f"-DORACLE={self.oracle}",
                f"-DRUN_DIR={output}",
                f"-DCONFIG={ROOT}/scripts/reference.cfg",
                f"-DINPUT={save}",
                f"-DREPLAY={actions}",
                "-P",
                str(ROOT / "scripts/check-replay-native.cmake"),
            ],
            case / "native-command",
        )
        return output

    def rust(self, save: Path, actions: Path, case: Path) -> Path:
        output = case / "rust"
        _ = run(
            [str(self.cli), "replay-world", str(save), str(actions), str(output)],
            case / "rust-command",
        )
        return output

    def compare_outputs(self, native: Path, rust: Path, case: Path) -> None:
        expected = read_json(native / "results.json")
        actual = read_json(rust / "results.json")
        labels = checkpoint_labels(expected)
        if checkpoint_labels(actual) != labels:
            raise WorldCheckError(f"Checkpoint labels differ: {case}")
        case.mkdir(parents=True, exist_ok=False)
        write_json(case / "native-results.json", deterministic(expected))
        write_json(case / "rust-results.json", actual)
        self.compare(
            case / "native-results.json",
            case / "rust-results.json",
            case / "results-compare",
        )
        for label in labels:
            for extension in ("world.json", "derived.json"):
                self.compare(
                    native / f"{label}.{extension}",
                    rust / f"{label}.{extension}",
                    case / f"{label}-{extension}-compare",
                )
            runtime = case / f"{label}.native.runtime.json"
            write_json(
                runtime, deterministic(read_json(native / f"{label}.runtime.json"))
            )
            self.compare(
                runtime,
                rust / f"{label}.runtime.json",
                case / f"{label}-runtime-compare",
            )
            exported = run(
                [str(self.cli), "world", str(rust / f"{label}.sav"), "--view", "saved"],
                case / f"{label}-save-export",
            )
            path = case / f"{label}.decoded-save.json"
            _ = path.write_text(exported.stdout)
            self.compare(
                native / f"{label}.world.json", path, case / f"{label}-save-compare"
            )

    def scenario(self, name: str, fixture: str) -> Path:
        case = self.artifacts / name
        save = FIXTURES / f"{fixture}-v362.sav"
        actions = FIXTURES / f"{name}.json"
        native = self.native(save, actions, case)
        rust = self.rust(save, actions, case)
        exported = run(
            [str(self.cli), "world", str(save), "--view", "saved"],
            case / "input-export",
        )
        path = case / "input.world.json"
        _ = path.write_text(exported.stdout)
        self.compare(
            path, native / "initial.world.json", case / "load-lifecycle-compare"
        )
        self.compare_outputs(native, rust, case / "compare")
        return case
