from __future__ import annotations

import hashlib
import tarfile
from pathlib import Path

from scripts.gameplay_foundations import digest
from scripts.grf_control_archive import archive_files
from scripts.world_check_support import WorldCheckError, at, read_json


def bounded_paths(root: Path, expected: set[str]) -> list[Path]:
    actual: set[str] = set()
    for path in root.rglob("*"):
        if path.is_symlink():
            raise WorldCheckError("Depot evidence contains a symlink")
        if path.is_file():
            actual.add(str(path.relative_to(root)))
    if actual != expected:
        raise WorldCheckError("Depot evidence membership differs from pinned layout")
    paths = [root / name for name in sorted(expected)]
    for path in paths:
        if not path.resolve().is_relative_to(root.resolve()) or not path.is_file():
            raise WorldCheckError("Depot evidence escaped its directory")
        if path.stat().st_size == 0 and path.name not in {"stdout.log", "stderr.log"}:
            raise WorldCheckError(f"Empty substantive depot evidence: {path}")
    return paths


def verify_archive(output: Path) -> None:
    index = read_json(output / "evidence-index.json")
    hashes = at(index, ("files",))
    match hashes:
        case dict() if hashes and len(hashes) == at(index, ("count",)):
            pass
        case _:
            raise WorldCheckError("Invalid depot archive index")
    for name, sha in hashes.items():
        path = output / name
        if (
            path.is_symlink()
            or not path.resolve().is_relative_to(output.resolve())
            or digest(path) != sha
        ):
            raise WorldCheckError(f"Depot archived raw evidence changed: {name}")
    destination = output / "evidence.tar.gz"
    if digest(destination) != at(index, ("archive_sha256",)):
        raise WorldCheckError("Depot archive digest changed")
    observed: set[str] = set()
    with tarfile.open(destination, "r:gz") as archive:
        for member in archive:
            if (
                not member.isfile()
                or member.name not in hashes
                or member.name in observed
            ):
                raise WorldCheckError("Depot archive member identity changed")
            source = archive.extractfile(member)
            if source is None:
                raise WorldCheckError("Depot archive member data missing")
            hasher = hashlib.sha256()
            with source:
                while chunk := source.read(1024 * 1024):
                    hasher.update(chunk)
            if hasher.hexdigest() != hashes[member.name]:
                raise WorldCheckError("Depot archive member bytes changed")
            observed.add(member.name)
    if observed != set(hashes):
        raise WorldCheckError("Depot archive member set incomplete")


def package_raw(output: Path) -> None:
    paths = [path for path in output.rglob("*") if path.is_file()]
    _ = bounded_paths(output, {str(path.relative_to(output)) for path in paths})
    archive_files(output, sorted(paths))
    verify_archive(output)
