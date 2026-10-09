from __future__ import annotations

import hashlib
import sys
import tarfile
from pathlib import Path

if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.world_check_support import Json, WorldCheckError, read_json, write_json

CONTROL_PATHS: tuple[tuple[str | int, ...], ...] = (
    ("metadata", "palette_bits"),
    ("metadata", "name", "entries", 0, "translated"),
    ("metadata", "parameters", 0, "default"),
    ("metadata", "parameters", 0, "complete_labels"),
    ("defaults", "parameters", 0),
    ("metadata", "name", "selections", 0, "text"),
    ("metadata", "name", "selections", 0, "language"),
    ("metadata", "compatibility", 0, "compatible"),
    ("failure", "line"),
)


def present(value: Json, path: tuple[str | int, ...]) -> bool:
    for part in path:
        match part, value:
            case str() as key, dict() as fields if key in fields:
                value = fields[key]
            case int() as index, list() as items if index < len(items):
                value = items[index]
            case _:
                return False
    return True


def expected_controls(native: Json) -> set[str]:
    match native:
        case {"scans": list() as scans, "texts": list() as texts}:
            if len(scans) != 1720 or len(texts) != 2446:
                raise WorldCheckError("Metadata matrix coverage changed")
        case _:
            raise WorldCheckError("Invalid native metadata observations")
    expected: set[str] = set()
    number = 0
    for index, scan in enumerate(scans):
        for pointer in CONTROL_PATHS:
            if present(scan, pointer):
                number += 1
                expected.add(f"negative-{index}-{number}.json")
    if number != 11456:
        raise WorldCheckError("Native metadata control coverage changed")
    return expected


def validate_controls(root: Path, expected: set[str]) -> list[Path]:
    actual = {path.name for path in root.glob("negative-*.json")}
    if actual != expected:
        raise WorldCheckError("Metadata control path set is incomplete or unexpected")
    paths = [root / name for name in sorted(expected)]
    for path in paths:
        if (
            path.resolve().parent != root.resolve()
            or not path.is_file()
            or path.stat().st_size == 0
        ):
            raise WorldCheckError(
                f"Missing, empty or escaped metadata control: {path.name}"
            )
    return paths


def digest(path: Path) -> str:
    with path.open("rb") as source:
        return hashlib.file_digest(source, "sha256").hexdigest()


def package(directory: Path) -> None:
    results = directory / "results"
    metadata = results / "native/metadata.json"
    expected = expected_controls(read_json(metadata))
    paths = validate_controls(results, expected)
    hashes = {"results/" + path.name: digest(path) for path in paths}
    destination = directory / "controls.tar"
    with tarfile.open(destination, "x") as archive:
        for path in paths:
            archive.add(path, arcname="results/" + path.name, recursive=False)
    observed: set[str] = set()
    with tarfile.open(destination, "r") as archive:
        for member in archive:
            if (
                not member.isfile()
                or member.name not in hashes
                or member.name in observed
            ):
                raise WorldCheckError("Metadata archive has an unexpected entry")
            content = archive.extractfile(member)
            if content is None:
                raise WorldCheckError("Metadata archive entry has no data")
            with content:
                actual = hashlib.sha256(content.read()).hexdigest()
            if actual != hashes[member.name]:
                raise WorldCheckError("Metadata archive differs from indexed control")
            observed.add(member.name)
    if observed != set(hashes):
        raise WorldCheckError("Metadata archive is incomplete")
    indexed_hashes: dict[str, Json] = dict(hashes)
    index: dict[str, Json] = {
        "schema_version": 1,
        "count": len(paths),
        "files": indexed_hashes,
        "native_metadata_sha256": digest(metadata),
        "archive_sha256": digest(destination),
    }
    write_json(directory / "controls-index.json", index)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise WorldCheckError("Expected one fresh metadata artifact directory")
    package(Path(sys.argv[1]).resolve())
