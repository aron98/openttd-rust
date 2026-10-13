"""Normalize only SHA-pinned literal artifact arrays into the existing model."""

import hashlib
from dataclasses import dataclass
from pathlib import Path
from typing import Final

from contract_model import (
    ContractError,
    Json,
    array,
    decode_json,
    parse_assets,
    record,
    strings,
    text,
)

LAYOUT_BYTES: Final = 1024 * 1024
EXTERNAL_BYTES: Final = 1024 * 1024
DRIVER_FIELDS: Final = (
    "id argv artifact_root artifact_globs success_text scopes timeout_seconds"
)


@dataclass(frozen=True, slots=True)
class ArtifactListRef:
    asset: str


def layout_path(root: Path, name: str) -> Path:
    relative = Path(name)
    if relative.is_absolute() or ".." in relative.parts:
        raise ContractError(f"invalid layout repository path: {name}")
    current = root.resolve()
    for part in relative.parts:
        current = current / part
        if current.is_symlink():
            raise ContractError(f"layout symlink is forbidden: {name}")
    if not current.resolve().is_relative_to(root.resolve()):
        raise ContractError(f"layout path escapes repository: {name}")
    if not current.is_file():
        raise ContractError(f"layout asset file missing: {name}")
    return current


def expand_artifact_lists(
    raw: dict[str, Json], root: Path
) -> tuple[dict[str, Json], dict[str, bytes]]:
    """Hash and parse one bounded buffer; charge every reference use, not only IDs."""
    assets = parse_assets(raw["assets"])
    remaining = EXTERNAL_BYTES
    verified: dict[str, bytes] = {}
    drivers: list[Json] = []
    for value in array(raw["drivers"]):
        row = record(value, DRIVER_FIELDS)
        declaration = row["artifact_globs"]
        match declaration:
            case list():
                drivers.append(row)
                continue
            case _:
                reference = ArtifactListRef(text(record(declaration, "asset")["asset"]))
        matching = [asset for asset in assets if asset.id == reference.asset]
        if len(matching) != 1:
            raise ContractError(f"missing or ambiguous layout asset: {reference.asset}")
        asset = matching[0]
        if (
            asset.kind != "evidence-layout"
            or asset.profile is not None
            or asset.save_version is not None
        ):
            raise ContractError(f"invalid layout asset metadata: {asset.id}")
        source = layout_path(root, asset.path)
        with source.open("rb") as handle:
            data = handle.read(min(LAYOUT_BYTES, remaining) + 1)
        if len(data) > LAYOUT_BYTES:
            raise ContractError("layout byte limit exceeded (1 MiB)")
        if len(data) > remaining:
            raise ContractError(
                "cumulative external layout byte limit exceeded (1 MiB)"
            )
        remaining -= len(data)
        if hashlib.sha256(data).hexdigest() != asset.sha256:
            raise ContractError(f"asset sha256 mismatch: {asset.id}")
        expanded = strings(decode_json(data))
        verified[asset.id] = data
        drivers.append({**row, "artifact_globs": list(expanded)})
    return {**raw, "drivers": drivers}, verified
