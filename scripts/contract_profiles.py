"""Link corpus content declarations to the independently recorded native profile."""

import hashlib
from pathlib import Path
import re
import tomllib

from contract_model import Contract, ContractError, array, read_json, record, text


def check_profiles(contract: Contract, root: Path) -> None:
    assets = {asset.id: asset for asset in contract.assets}
    for profile in contract.profiles:
        if not profile.settings or not profile.settings_assets:
            raise ContractError(f"empty settings profile: {profile.id}")
        if set(profile.settings_assets) - assets.keys():
            raise ContractError(f"unknown settings asset: {profile.id}")
        for content in profile.content:
            if content.asset not in assets:
                raise ContractError(f"unknown content asset: {content.asset}")
            data = (root / assets[content.asset].path).read_bytes()
            if hashlib.md5(data, usedforsecurity=False).hexdigest() != content.md5:
                raise ContractError(f"content md5 mismatch: {content.asset}")
            if re.fullmatch(r"[0-9a-f]{8}", content.grfid) is None:
                raise ContractError(f"invalid grfid: {content.asset}")
            if any(p < 0 or p > 0xFFFFFFFF for p in content.parameters):
                raise ContractError(f"content parameter outside u32: {content.asset}")
        if profile.content_metadata is None:
            if profile.content:
                raise ContractError(
                    f"content identity requires native metadata: {profile.id}"
                )
            continue
        if profile.content_metadata not in assets:
            raise ContractError(f"unknown content metadata asset: {profile.id}")
        metadata = record(
            read_json(root / assets[profile.content_metadata].path),
            "schema_version license upstream_commit savegame_version assets newgrf ai effect command limits",
        )
        upstream = tomllib.loads((root / "upstream.toml").read_text())
        if (
            metadata["upstream_commit"] != upstream["commit"]
            or metadata["savegame_version"] != upstream["savegame_version"]
        ):
            raise ContractError(f"native metadata pin mismatch: {profile.id}")
        for row in array(metadata["assets"]):
            entry = record(row, "path sha256")
            path = text(entry["path"])
            if path.endswith(".sav"):
                matching = [asset for asset in contract.assets if asset.path == path]
                if (
                    len(matching) != 1
                    or matching[0].profile != profile.id
                    or matching[0].sha256 != entry["sha256"]
                ):
                    raise ContractError(f"native save profile mismatch: {path}")
        original = record(
            metadata["newgrf"],
            "grfid grfid_byte_order md5 parameters load_order name version",
        )
        if len(profile.content) != 1:
            raise ContractError(
                f"content identity differs from single-GRF native baseline: {profile.id}"
            )
        content = profile.content[0]
        if (
            content.grfid != original["grfid"]
            or content.md5 != original["md5"]
            or list(content.parameters) != original["parameters"]
            or [assets[content.asset].path] != original["load_order"]
        ):
            raise ContractError(
                f"content identity differs from native metadata: {profile.id}"
            )
