from __future__ import annotations

import hashlib
import tarfile
from pathlib import Path

from scripts.gameplay_foundations import digest
from scripts.world_check_support import Json, WorldCheckError, write_json


def validate_archive_paths(directory: Path, paths: list[Path]) -> None:
    for path in paths:
        if path.is_symlink() or not path.resolve().is_relative_to(directory.resolve()):
            raise WorldCheckError("Loader archive path escaped evidence directory")
        if not path.stat().st_size and path.name not in {"stdout.log", "stderr.log"}:
            raise WorldCheckError(f"Empty substantive loader archive artifact: {path}")


def archive_files(directory: Path, paths: list[Path]) -> None:
    validate_archive_paths(directory, paths)
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
