"""Retain an exact first archive and actual serialized archive admission corruptions."""

from __future__ import annotations

import hashlib
import tarfile
from pathlib import Path

from .baseline import FileInput
from .native import write
from .value import DECODE, EvidenceError, Json, field, integer, require


def stream(archive: Path, index: Json) -> None:
    require(
        "archive byte identity",
        condition=FileInput.capture(archive).digest == field(index, "archive_sha256"),
    )
    files = field(index, "files")
    require("nonempty archive file index", condition=isinstance(files, dict))
    if not isinstance(files, dict):
        raise EvidenceError("archive file index object")
    require(
        "archive count",
        condition=bool(files) and len(files) == integer(field(index, "count")),
    )
    observed: set[str] = set()
    with tarfile.open(archive, "r:gz") as opened:
        for member in opened:
            require(
                "archive member identity",
                condition=member.isfile()
                and member.name in files
                and member.name not in observed,
            )
            content = opened.extractfile(member)
            require("archive member stream", condition=content is not None)
            if content is None:
                raise EvidenceError("archive member stream")
            hasher = hashlib.sha256()
            with content:
                while chunk := content.read(1024 * 1024):
                    hasher.update(chunk)
            digest = hasher.hexdigest()
            require("archive member bytes", condition=digest == files[member.name])
            observed.add(member.name)
    require("archive member set", condition=observed == set(files))


def refuse(directory: Path, name: str, index: Json, archive: Path) -> Json:
    destination = directory / (name + ".json")
    write(destination, index)
    try:
        stream(archive, DECODE(destination.read_text()))
    except EvidenceError as error:
        return {"artifact": destination.name, "diagnostic": str(error)}
    raise EvidenceError("archive corruption admitted: " + name)


def run(output: Path) -> None:
    directory = output / "archive-controls"
    directory.mkdir()
    archive = directory / "original.tar.gz"
    index_path = directory / "original-index.json"
    _ = (output / "evidence.tar.gz").rename(archive)
    _ = (output / "evidence-index.json").rename(index_path)
    index = DECODE(index_path.read_text())
    stream(archive, index)
    require("archive index object", condition=isinstance(index, dict))
    if not isinstance(index, dict):
        raise EvidenceError("archive index object")
    files = field(index, "files")
    require("archive files object", condition=isinstance(files, dict))
    if not isinstance(files, dict):
        raise EvidenceError("archive files object")
    first = next(iter(files))
    mutations: tuple[tuple[str, Json], ...] = (
        ("member_hash", {**index, "files": {**files, first: "0" * 64}}),
        (
            "missing_member",
            {
                **index,
                "count": len(files) - 1,
                "files": {k: v for k, v in files.items() if k != first},
            },
        ),
        (
            "extra_member",
            {
                **index,
                "count": len(files) + 1,
                "files": {**files, "never-produced": "0" * 64},
            },
        ),
    )
    rows = [refuse(directory, name, value, archive) for name, value in mutations]
    changed = directory / "truncated.tar.gz"
    with archive.open("rb") as source:
        prefix = source.read(1024)
    _ = changed.write_bytes(prefix[:-1])
    rows.append(refuse(directory, "archive_bytes", index, changed))
    write(directory / "summary.json", {"actual_serialized_refusals": rows})
    verify(output)


def verify(output: Path) -> None:
    directory = output / "archive-controls"
    archive = directory / "original.tar.gz"
    stream(archive, DECODE((directory / "original-index.json").read_text()))
    summary = DECODE((directory / "summary.json").read_text())
    require(
        "archive controls summary",
        condition=isinstance(field(summary, "actual_serialized_refusals"), list),
    )
    names = ("member_hash", "missing_member", "extra_member", "archive_bytes")
    require(
        "exact archive control membership",
        condition={p.name for p in directory.iterdir()}
        == {
            "original.tar.gz",
            "original-index.json",
            "truncated.tar.gz",
            "summary.json",
            *(name + ".json" for name in names),
        },
    )
    for name in names:
        target = directory / "truncated.tar.gz" if name == "archive_bytes" else archive
        try:
            stream(target, DECODE((directory / (name + ".json")).read_text()))
        except EvidenceError:
            continue
        raise EvidenceError("retained archive corruption admitted: " + name)
