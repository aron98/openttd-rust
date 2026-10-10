# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through scripts/check-script-vm.py.
"""Pinned source export and Cargo executable identities."""

import hashlib
from dataclasses import dataclass
from pathlib import Path

from scripts.world_check_support import Json, WorldCheckError, decode_json, read_json

PIN = "14ec60f248547d4d062a1160f0fc26d742319888"


@dataclass(frozen=True, slots=True)
class Spec:
    fixtures: tuple[str, ...]
    credits: tuple[tuple[int, ...], ...]
    tests: tuple[str, ...]
    sources: tuple[tuple[str, str], ...]
    native: tuple[tuple[str, str], ...]
    stages: tuple[tuple[str, str], ...]
    paths: tuple[str, ...]


def strings(value: Json) -> tuple[str, ...]:
    match value:
        case list() as items if all(isinstance(x, str) for x in items):
            return tuple(x for x in items if isinstance(x, str))
        case _:
            raise WorldCheckError("Expected string membership")


def pairs(value: Json) -> tuple[tuple[str, str], ...]:
    match value:
        case dict() as entries if all(isinstance(x, str) for x in entries.values()):
            return tuple((k, v) for k, v in entries.items() if isinstance(v, str))
        case _:
            raise WorldCheckError("Expected source identity map")


def load_spec(path: Path) -> Spec:
    match read_json(path):
        case {
            "pin": str() as pin,
            "fixtures": fixtures,
            "credits": list() as credits,
            "tests": tests,
            "sources": sources,
            "native": native,
            "stages": stages,
            "paths": paths,
        } if pin == PIN:
            sequences: list[tuple[int, ...]] = []
            for row in credits:
                match row:
                    case list() as values if values and all(
                        type(v) is int and 0 <= v <= 10000 for v in values
                    ):
                        sequences.append(tuple(v for v in values if isinstance(v, int)))
                    case _:
                        raise WorldCheckError("Invalid credit sequence")
            spec = Spec(
                strings(fixtures),
                tuple(sequences),
                strings(tests),
                pairs(sources),
                pairs(native),
                pairs(stages),
                strings(paths),
            )
        case _:
            raise WorldCheckError("Invalid VM evidence manifest")
    if (
        len(set(spec.fixtures)),
        len(set(spec.credits)),
        len(set(spec.tests)),
        len(spec.stages),
    ) != (1188, 9, 62, 10692) or len(set(spec.paths)) != len(spec.paths):
        raise WorldCheckError("Incomplete VM manifest membership")
    return spec


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def blob_digest(path: Path) -> str:
    data = path.read_bytes()
    return hashlib.sha1(b"blob " + str(len(data)).encode() + b"\0" + data).hexdigest()


def check_blob(path: Path, expected: str) -> None:
    if path.is_symlink() or blob_digest(path) != expected:
        raise WorldCheckError(f"Native source differs from pinned Git blob: {path}")


def verify_sources(root: Path, native: Path, spec: Spec) -> None:
    for name, sha in spec.sources:
        if digest(root / name) != sha:
            raise WorldCheckError(f"VM source changed: {name}")
    actual = {
        str(p.relative_to(native)) for p in (native / "src").rglob("*") if p.is_file()
    }
    if actual != {p for p, _ in spec.native}:
        raise WorldCheckError("Native source membership differs")
    for name, blob in spec.native:
        check_blob(native / name, blob)
    expected = f"pin={PIN}\ninteger_bits=64\nfloat_bits=32\ngc=enabled\n"
    if (native / "variant.txt").read_text() != expected:
        raise WorldCheckError("Native variant differs")
    for name in ("observe.cpp", "strings.hpp", "constants.hpp"):
        if digest(native / name) != digest(root / "scripts/compat/script-vm" / name):
            raise WorldCheckError("Native observer source differs")


def select_executable(output: str, name: str, kind: str, test: bool) -> Path:
    candidates: list[Path] = []
    for line in output.splitlines():
        match decode_json(line):
            case {
                "reason": "compiler-artifact",
                "executable": str() as executable,
                "target": {"name": str() as found, "kind": list() as kinds},
                "profile": {"test": bool() as testing},
            } if found == name:
                if kinds != [kind] or testing != test:
                    raise WorldCheckError("Incorrect Cargo executable kind/profile")
                candidates.append(Path(executable))
            case _:
                continue
    if len(candidates) != 1:
        raise WorldCheckError("Missing/ambiguous Cargo executable")
    return candidates[0].resolve()


def archive_files(directory: Path, paths: tuple[str, ...]) -> None:
    import tarfile

    from scripts.world_check_support import write_json

    hashes: dict[str, Json] = {name: digest(directory / name) for name in paths}
    destination = directory / "evidence.tar.gz"
    with tarfile.open(destination, "x:gz") as archive:
        for name in paths:
            archive.add(directory / name, arcname=name, recursive=False)
    verify_archive(directory, paths)
    write_json(
        directory / "evidence-index.json",
        {"files": hashes, "count": len(paths), "archive_sha256": digest(destination)},
    )


def verify_archive(directory: Path, paths: tuple[str, ...]) -> None:
    import tarfile

    hashes = {name: digest(directory / name) for name in paths}
    destination = directory / "evidence.tar.gz"
    seen: set[str] = set()
    with tarfile.open(destination, "r:gz") as archive:
        for member in archive:
            if not member.isfile() or member.name in seen or member.name not in hashes:
                raise WorldCheckError("Invalid VM archive membership")
            source = archive.extractfile(member)
            if source is None:
                raise WorldCheckError("Missing VM archive bytes")
            with source:
                if hashlib.sha256(source.read()).hexdigest() != hashes[member.name]:
                    raise WorldCheckError("VM archive hash differs")
            seen.add(member.name)
    if seen != set(paths):
        raise WorldCheckError("Incomplete VM archive")
