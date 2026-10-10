"""Actual serialized identity corruptions bound to selected producer artifacts."""

from __future__ import annotations

import shutil
from pathlib import Path

from scripts.script_vm_provenance import select_executable
from scripts.world_check_support import WorldCheckError

from .baseline import FileInput
from .native import Native, write
from .rust import Rust
from .value import DECODE, EvidenceError, Json, array, field, require, string


def run(output: Path, rust: Rust, native: Native) -> None:
    directory = output / "binding-corruption"
    directory.mkdir()
    rows: list[Json] = []
    for name, source in (
        ("native", native.executable),
        ("rust", rust.harness.retained),
        ("config", native.config),
        ("source", native.closure[0]),
    ):
        source.verify()
        artifact = directory / name
        _ = shutil.copyfile(source.path, artifact)
        with artifact.open("r+b") as stream:
            first = stream.read(1)
            require("binding mutation needs nonempty source", condition=bool(first))
            _ = stream.seek(0)
            _ = stream.write(bytes((first[0] ^ 1,)))
        try:
            require(
                "bound artifact digest changed",
                condition=FileInput.capture(artifact).digest == source.digest,
            )
        except EvidenceError as error:
            rows.append(
                {
                    "artifact": name,
                    "source": str(source.path),
                    "expected_sha256": source.digest,
                    "mutated_sha256": FileInput.capture(artifact).digest,
                    "diagnostic": str(error),
                }
            )
        else:
            raise EvidenceError("mutated producer identity admitted")
        source.verify()
    cargo = directory / "cargo-empty.jsonl"
    _ = cargo.write_text('{"reason":"build-finished","success":true}\n')
    try:
        _ = select_executable(
            cargo.read_text(), rust.harness.target, rust.harness.kind, test=True
        )
    except WorldCheckError as error:
        rows.append({"artifact": cargo.name, "diagnostic": str(error)})
    else:
        raise EvidenceError("missing actual Cargo executable admitted")
    write(directory / "summary.json", {"actual_mutations": rows})


def verify(output: Path) -> None:
    directory = output / "binding-corruption"
    summary = DECODE((directory / "summary.json").read_text())
    rows = array(field(summary, "actual_mutations"))
    require(
        "binding corruption roster",
        condition=[field(row, "artifact") for row in rows]
        == ["native", "rust", "config", "source", "cargo-empty.jsonl"],
    )
    for row in rows[:4]:
        item = FileInput.capture(directory / string(field(row, "artifact")))
        require(
            "mutation retained exactly",
            condition=item.digest == field(row, "mutated_sha256")
            and item.digest != field(row, "expected_sha256"),
        )
    try:
        _ = select_executable(
            (directory / "cargo-empty.jsonl").read_text(),
            "road_movement_ci",
            "test",
            test=True,
        )
    except WorldCheckError:
        return
    raise EvidenceError("Cargo mutation admitted")
