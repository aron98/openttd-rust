from __future__ import annotations

import hashlib
import tarfile
from pathlib import Path
from typing import Final

from scripts.gameplay_foundations import digest
from scripts.world_check_support import Json, WorldCheckError, write_json

NO_EMPTY_INPUTS: Final[frozenset[tuple[str, str]]] = frozenset()


def validate_empty_inputs(
    directory: Path, paths: list[Path], allowances: frozenset[tuple[str, str]]
) -> set[str]:
    available = {str(path.relative_to(directory)): path for path in paths}
    allowed: set[str] = set()
    empty_sha = hashlib.sha256(b"").hexdigest()
    for name, sha in allowances:
        if name in allowed or name not in available or sha != empty_sha:
            raise WorldCheckError("Unused or mismatched empty-input allowance")
        path = available[name]
        if path.stat().st_size != 0 or digest(path) != sha:
            raise WorldCheckError("Declared empty input is not the pinned empty file")
        allowed.add(name)
    return allowed


def validate_archive_paths(
    directory: Path,
    paths: list[Path],
    *,
    empty_inputs: frozenset[tuple[str, str]] = NO_EMPTY_INPUTS,
) -> None:
    allowed = validate_empty_inputs(directory, paths, empty_inputs)
    for path in paths:
        if path.is_symlink() or not path.resolve().is_relative_to(directory.resolve()):
            raise WorldCheckError("Loader archive path escaped evidence directory")
        if any(
            character in '\\:"<>|*?' or ord(character) < 32
            for character in str(path.relative_to(directory))
        ):
            raise WorldCheckError(f"Evidence path is not upload portable: {path}")
        if (
            not path.stat().st_size
            and path.name not in {"stdout.log", "stderr.log"}
            and str(path.relative_to(directory)) not in allowed
        ):
            raise WorldCheckError(f"Empty substantive loader archive artifact: {path}")


def archive_files(
    directory: Path,
    paths: list[Path],
    *,
    empty_inputs: frozenset[tuple[str, str]] = NO_EMPTY_INPUTS,
) -> None:
    validate_archive_paths(directory, paths, empty_inputs=empty_inputs)
    destination = directory / "evidence.tar.gz"
    hashes: dict[str, Json] = {
        str(path.relative_to(directory)): digest(path) for path in paths
    }
    with tarfile.open(destination, "x:gz") as archive:
        for path in paths:
            archive.add(path, arcname=str(path.relative_to(directory)), recursive=False)
    observed: set[str] = set()
    with tarfile.open(destination, "r:gz") as archive:
        for member in archive:
            if (
                not member.isfile()
                or member.name not in hashes
                or member.name in observed
            ):
                raise WorldCheckError("Loader archive membership changed")
            source = archive.extractfile(member)
            if source is None:
                raise WorldCheckError("Loader archive data missing")
            with source:
                hasher = hashlib.sha256()
                while chunk := source.read(1024 * 1024):
                    hasher.update(chunk)
                actual = hasher.hexdigest()
            if actual != hashes[member.name]:
                raise WorldCheckError("Loader archive content changed")
            observed.add(member.name)
    if observed != set(hashes):
        raise WorldCheckError("Loader archive is incomplete")
    write_json(
        directory / "evidence-index.json",
        {
            "schema_version": 1,
            "files": hashes,
            "count": len(paths),
            "archive_sha256": digest(destination),
        },
    )
