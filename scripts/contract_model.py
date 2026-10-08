"""Typed metadata boundary for the compatibility contract."""

from dataclasses import dataclass
from enum import StrEnum
import json
from pathlib import Path
from typing import TypeAlias

Json: TypeAlias = None | bool | int | float | str | list["Json"] | dict[str, "Json"]


class ContractError(Exception):
    """Invalid contract or evidence; the diagnostic identifies its field."""


class Status(StrEnum):
    IMPLEMENTED = "implemented"
    REFERENCE = "reference_only"
    FUTURE = "unimplemented"


class Scope(StrEnum):
    RUST = "rust_behavior"
    CONTAINER = "container_preservation"
    ORIGINAL = "original_only"
    FUTURE = "future"


@dataclass(frozen=True, slots=True)
class Asset:
    id: str
    path: str
    sha256: str
    kind: str
    provenance: str
    save_version: int | None
    profile: str | None


@dataclass(frozen=True, slots=True)
class Content:
    asset: str
    grfid: str
    md5: str
    parameters: tuple[int, ...]


@dataclass(frozen=True, slots=True)
class Profile:
    id: str
    settings: tuple[str, ...]
    settings_assets: tuple[str, ...]
    content_metadata: str | None
    content: tuple[Content, ...]


@dataclass(frozen=True, slots=True)
class Driver:
    id: str
    argv: tuple[str, ...]
    artifact_root: str | None
    artifact_globs: tuple[str, ...]
    success_text: str
    scopes: tuple[Scope, ...]
    timeout_seconds: int


@dataclass(frozen=True, slots=True)
class Scenario:
    id: str
    status: Status
    scope: Scope
    domain: str
    expected: str
    driver: str | None
    assets: tuple[str, ...]
    source_refs: tuple[str, ...]
    evidence: tuple[str, ...]


@dataclass(frozen=True, slots=True)
class Contract:
    manifest_path: str
    manifest_sha256: str
    assets: tuple[Asset, ...]
    profiles: tuple[Profile, ...]
    drivers: tuple[Driver, ...]
    scenarios: tuple[Scenario, ...]


def unique_object(pairs: list[tuple[str, Json]]) -> dict[str, Json]:
    result: dict[str, Json] = {}
    for key, value in pairs:
        if key in result:
            raise ContractError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def read_json(path: Path) -> Json:
    with path.open("rb") as source:
        data = source.read(1024 * 1024 + 1)
    if len(data) > 1024 * 1024:
        raise ContractError("manifest byte limit exceeded (1 MiB)")
    try:
        return json.loads(data, object_pairs_hook=unique_object)
    except (UnicodeError, RecursionError) as error:
        raise ContractError(f"invalid JSON encoding/depth: {error}") from error


def record(value: Json, fields: str) -> dict[str, Json]:
    if not isinstance(value, dict):
        raise ContractError("expected object")
    expected = set(fields.split())
    if set(value) != expected:
        raise ContractError(f"unknown or missing fields: {set(value) ^ expected}")
    return value


def text(value: Json) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ContractError("expected nonempty string")
    return value


def optional_text(value: Json) -> str | None:
    return None if value is None else text(value)


def integer(value: Json) -> int:
    if not isinstance(value, int) or isinstance(value, bool):
        raise ContractError("expected integer")
    return value


def array(value: Json) -> list[Json]:
    if not isinstance(value, list):
        raise ContractError("expected array")
    return value


def strings(value: Json) -> tuple[str, ...]:
    return tuple(text(item) for item in array(value))


def parse_contract(raw: dict[str, Json]) -> Contract:
    assets = []
    for item in array(raw["assets"]):
        row = record(item, "id path sha256 kind provenance save_version profile")
        version = row["save_version"]
        assets.append(
            Asset(
                text(row["id"]),
                text(row["path"]),
                text(row["sha256"]),
                text(row["kind"]),
                text(row["provenance"]),
                None if version is None else integer(version),
                optional_text(row["profile"]),
            )
        )
    profiles = []
    for item in array(raw["profiles"]):
        row = record(item, "id settings settings_assets content_metadata content")
        contents = []
        for entry in array(row["content"]):
            content = record(entry, "asset grfid md5 parameters")
            contents.append(
                Content(
                    text(content["asset"]),
                    text(content["grfid"]),
                    text(content["md5"]),
                    tuple(integer(p) for p in array(content["parameters"])),
                )
            )
        profiles.append(
            Profile(
                text(row["id"]),
                strings(row["settings"]),
                strings(row["settings_assets"]),
                optional_text(row["content_metadata"]),
                tuple(contents),
            )
        )
    drivers = []
    for item in array(raw["drivers"]):
        row = record(
            item,
            "id argv artifact_root artifact_globs success_text scopes timeout_seconds",
        )
        try:
            scopes = tuple(Scope(value) for value in strings(row["scopes"]))
        except ValueError as error:
            raise ContractError(f"invalid driver scope: {error}") from error
        drivers.append(
            Driver(
                text(row["id"]),
                strings(row["argv"]),
                optional_text(row["artifact_root"]),
                strings(row["artifact_globs"]),
                text(row["success_text"]),
                scopes,
                integer(row["timeout_seconds"]),
            )
        )
    scenarios = []
    for item in array(raw["scenarios"]):
        row = record(
            item, "id status scope domain expected driver assets source_refs evidence"
        )
        try:
            status, scope = Status(text(row["status"])), Scope(text(row["scope"]))
        except ValueError as error:
            raise ContractError(f"invalid status/scope: {error}") from error
        scenarios.append(
            Scenario(
                text(row["id"]),
                status,
                scope,
                text(row["domain"]),
                text(row["expected"]),
                optional_text(row["driver"]),
                strings(row["assets"]),
                strings(row["source_refs"]),
                strings(row["evidence"]),
            )
        )
    return Contract(
        "", "", tuple(assets), tuple(profiles), tuple(drivers), tuple(scenarios)
    )
