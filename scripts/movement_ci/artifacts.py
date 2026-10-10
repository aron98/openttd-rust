"""Exact archive membership and independently retained source/executable bytes."""

from __future__ import annotations

import hashlib
import shutil
from pathlib import Path

from scripts.depot_build_archive import package_raw, verify_archive

from .archive_controls import stream
from .baseline import FileInput
from .native import write
from .value import DECODE, Json, array, field, integer, require, string


def retain(directory: Path, inputs: tuple[FileInput, ...]) -> None:
    directory.mkdir()
    rows: list[Json] = []
    for index, item in enumerate(inputs):
        item.verify()
        target = directory / f"{index:05d}-{item.path.name}"
        _ = shutil.copy2(item.path, target)
        copied = FileInput.capture(target)
        require("retained input differs", condition=copied.digest == item.digest)
        rows.append(
            {
                "original": str(item.path),
                "retained": target.name,
                "sha256": item.digest,
                "bytes": item.size,
            }
        )
    write(directory / "inputs.json", rows)


def verify_retained(directory: Path) -> None:
    rows = array(DECODE((directory / "inputs.json").read_text()))
    expected = {"inputs.json"}
    for row in rows:
        name = string(field(row, "retained"))
        require(
            "unsafe retained name",
            condition=Path(name).name == name and name not in expected,
        )
        expected.add(name)
        item = FileInput.capture(directory / name)
        require(
            "retained byte identity",
            condition=item.digest == field(row, "sha256")
            and item.size == integer(field(row, "bytes")),
        )
    require(
        "retained input membership",
        condition={p.name for p in directory.iterdir()} == expected,
    )


def seal(output: Path) -> None:
    empties: set[tuple[str, str]] = set()
    for result_path in output.glob("*/results.json"):
        result = DECODE(result_path.read_text())
        protocol = DECODE((result_path.parent / "protocol.json").read_text())
        if field(protocol, "trace") is False:
            require(
                "trace-off must be successful",
                condition=field(result, "outcome") == "native_completed"
                and field(result, "events") == 0,
            )
            events = result_path.parent / "events.jsonl"
            require(
                "trace-off stream must be empty", condition=events.read_bytes() == b""
            )
            empties.add(
                (str(events.relative_to(output)), hashlib.sha256(b"").hexdigest())
            )
    for folder in ("sources", "native-inputs", "baseline-inputs"):
        verify_retained(output / folder)
        for row in array(DECODE((output / folder / "inputs.json").read_text())):
            if integer(field(row, "bytes")) == 0:
                empties.add(
                    (
                        folder + "/" + string(field(row, "retained")),
                        hashlib.sha256(b"").hexdigest(),
                    )
                )
    package_raw(output, empty_inputs=frozenset(empties))
    verify(output)


def verify(output: Path) -> None:
    verify_archive(output)
    index = DECODE((output / "evidence-index.json").read_text())
    stream(output / "evidence.tar.gz", index)
    hashes = field(index, "files")
    require("archive index object", condition=isinstance(hashes, dict))
    if not isinstance(hashes, dict):
        return
    expected = set(hashes) | {"evidence.tar.gz", "evidence-index.json"}
    require(
        "unindexed or missing raw file",
        condition={str(p.relative_to(output)) for p in output.rglob("*") if p.is_file()}
        == expected,
    )
    for name in ("sources", "native-inputs", "baseline-inputs"):
        verify_retained(output / name)
