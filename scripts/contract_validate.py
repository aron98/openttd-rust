"""Offline identity, reference and capability checks; never an execution pass."""

import hashlib
import re
import tomllib
from dataclasses import replace
from pathlib import Path
from typing import assert_never

from contract_layouts import expand_artifact_lists
from contract_model import (
    Contract,
    ContractError,
    Scope,
    Status,
    parse_contract,
    read_json,
    record,
)
from contract_profiles import check_profiles


def local_path(root: Path, name: str) -> Path:
    path = Path(name)
    if path.is_absolute() or ".." in path.parts:
        raise ContractError(f"invalid repository path: {name}")
    result = (root / path).resolve()
    if not result.is_relative_to(root.resolve()):
        raise ContractError(f"path escapes repository: {name}")
    return result


def unique_ids(ids: tuple[str, ...], kind: str) -> None:
    if len(ids) != len(set(ids)):
        raise ContractError(f"duplicate {kind} id")
    if any(re.fullmatch(r"[a-z0-9][a-z0-9.-]*", value) is None for value in ids):
        raise ContractError(f"invalid {kind} id")


def check_references(contract: Contract, root: Path) -> None:
    asset_ids = {asset.id for asset in contract.assets}
    drivers = {driver.id: driver for driver in contract.drivers}
    for scenario in contract.scenarios:
        if set(scenario.assets) - asset_ids:
            raise ContractError(f"unknown asset in {scenario.id}")
        match scenario.status:
            case Status.IMPLEMENTED:
                if scenario.scope not in (Scope.RUST, Scope.CONTAINER):
                    raise ContractError(f"implemented scope mismatch: {scenario.id}")
            case Status.REFERENCE:
                if scenario.scope is not Scope.ORIGINAL:
                    raise ContractError(f"reference_only scope mismatch: {scenario.id}")
            case Status.FUTURE:
                if (
                    scenario.driver is not None
                    or scenario.evidence
                    or scenario.scope is not Scope.FUTURE
                ):
                    raise ContractError(
                        f"unimplemented scenario cannot have executable evidence: {scenario.id}"
                    )
            case unreachable:
                assert_never(unreachable)
        if scenario.status is not Status.FUTURE:
            if scenario.driver not in drivers:
                raise ContractError(f"unknown driver in {scenario.id}")
            if not scenario.evidence or not scenario.source_refs:
                raise ContractError(
                    f"missing evidence/source references: {scenario.id}"
                )
            if scenario.scope not in drivers[scenario.driver].scopes:
                raise ContractError(f"driver evidence scope mismatch: {scenario.id}")
            declared = set(drivers[scenario.driver].artifact_globs) | {
                "stdout.log",
                "stderr.log",
            }
            if set(scenario.evidence) - declared:
                raise ContractError(f"evidence not required by driver: {scenario.id}")
        for reference in scenario.source_refs:
            path, separator, symbol = reference.partition("#")
            source = local_path(root, path)
            if not source.is_file() or (separator and symbol not in source.read_text()):
                raise ContractError(f"unresolved source reference: {reference}")


def load_contract(path: Path, root: Path) -> Contract:
    raw = record(
        read_json(path), "schema_version target assets profiles drivers scenarios"
    )
    if type(raw["schema_version"]) is not int or raw["schema_version"] != 1:
        raise ContractError("unsupported schema_version")
    target = record(
        raw["target"],
        "repository release commit savegame_version license protocol base_graphics",
    )
    upstream = tomllib.loads((root / "upstream.toml").read_text())
    for key, value in upstream.items():
        if target[key] != value or type(target[key]) is not type(value):
            raise ContractError(f"upstream pin mismatch: {key}")
    expected_protocol = {
        "game_info": 7,
        "admin": 3,
        "coordinator": 6,
        "survey": 2,
        "revision": "15.3",
        "query_packet": 7,
        "response_packet": 6,
    }
    if target["protocol"] != expected_protocol:
        raise ContractError("protocol pin mismatch for pinned 15.3 source")
    base_graphics = {
        "name": "OpenGFX",
        "version": "7.1",
        "archive_sha256": "928fcf34efd0719a3560cbab6821d71ce686b6315e8825360fba87a7a94d7846",
    }
    setup = (root / "scripts/setup-reference.sh").read_text()
    if (
        target["base_graphics"] != base_graphics
        or base_graphics["archive_sha256"] not in setup
    ):
        raise ContractError("base graphics pin mismatch")
    native_config = root / ".reference/OpenTTD/src/network/core/config.h"
    if native_config.is_file():
        source = native_config.read_text()
        for name, value in [
            ("NETWORK_GAME_INFO_VERSION", 7),
            ("NETWORK_GAME_ADMIN_VERSION", 3),
            ("NETWORK_COORDINATOR_VERSION", 6),
            ("NETWORK_SURVEY_VERSION", 2),
        ]:
            if re.search(rf"{name}\s*=\s*{value}\s*;", source) is None:
                raise ContractError(f"native protocol source pin mismatch: {name}")
    expanded, layout_bytes = expand_artifact_lists(raw, root)
    contract = replace(
        parse_contract(expanded),
        manifest_path=str(path.resolve()),
        manifest_sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
    )
    for kind, rows in [
        ("asset", contract.assets),
        ("profile", contract.profiles),
        ("driver", contract.drivers),
        ("scenario", contract.scenarios),
    ]:
        unique_ids(tuple(row.id for row in rows), kind)
        if not rows:
            raise ContractError(f"empty {kind} inventory")
    profile_ids = {profile.id for profile in contract.profiles}
    for asset in contract.assets:
        source = local_path(root, asset.path)
        if not source.is_file():
            raise ContractError(f"asset file missing: {asset.path}")
        verified = layout_bytes.get(asset.id)
        content = source.read_bytes() if verified is None else verified
        if hashlib.sha256(content).hexdigest() != asset.sha256:
            raise ContractError(f"asset sha256 mismatch: {asset.id}")
        if asset.profile is not None and asset.profile not in profile_ids:
            raise ContractError(f"unknown profile: {asset.id}")
        if asset.save_version is not None:
            header = source.read_bytes()[:8]
            if (
                len(header) < 8
                or int.from_bytes(header[4:6], "big") != asset.save_version
            ):
                raise ContractError(f"save_version mismatch: {asset.id}")
    check_profiles(contract, root)
    for driver in contract.drivers:
        if not 1 <= driver.timeout_seconds <= 7200:
            raise ContractError(f"invalid driver timeout_seconds: {driver.id}")
        if (
            not driver.argv
            or not driver.artifact_globs
            and driver.artifact_root is not None
        ):
            raise ContractError(f"incomplete driver: {driver.id}")
        for argument in driver.argv:
            if (
                argument.startswith("scripts/")
                and not local_path(root, argument).is_file()
            ):
                raise ContractError(f"driver script missing: {argument}")
        for pattern in driver.artifact_globs:
            local_path(root, pattern)
        if driver.artifact_root is not None:
            local_path(root, driver.artifact_root)
    check_references(contract, root)
    return contract
