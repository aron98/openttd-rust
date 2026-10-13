# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through scripts.sale_driver.
"""Cargo executable and source identities for bounded sale evidence."""

from __future__ import annotations

import os
import re
import shutil
import sys
from pathlib import Path

if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.gameplay_foundations import FoundationRun
from scripts.grf_metadata_evidence import digest
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    decode_json,
    read_json,
    write_json,
)


def prepare(directory: Path, oracle: Path) -> None:
    root = Path(__file__).resolve().parents[1]
    result = FoundationRun(root, directory, oracle).run(
        "build-sale-tests",
        [
            "cargo",
            "test",
            "--locked",
            "-p",
            "ottd-sim",
            "--lib",
            "--test",
            "native_sale",
            "--no-run",
            "--message-format=json",
        ],
    )
    binaries: dict[str, Json] = {}
    for line in result.stdout.splitlines():
        match decode_json(line):
            case {
                "reason": "compiler-artifact",
                "executable": str() as executable,
                "profile": {"test": True},
                "target": {"name": str() as name, "kind": list() as kind},
            } if name in ("ottd_sim", "native_sale"):
                if (name, kind) not in (
                    ("ottd_sim", ["lib"]),
                    ("native_sale", ["test"]),
                ):
                    raise WorldCheckError("Incorrect Cargo sale artifact kind")
                if name in binaries:
                    raise WorldCheckError("Ambiguous sale test executable")
                path = Path(executable).resolve(strict=True)
                retained = directory / "executables" / name
                retained.parent.mkdir(exist_ok=True)
                _ = shutil.copy2(path, retained)
                retained.chmod(0o555)
                binaries[name] = {
                    "path": str(path),
                    "sha256": digest(path),
                    "retained": str(retained),
                }
            case _:
                continue
    if set(binaries) != {"ottd_sim", "native_sale"}:
        raise WorldCheckError("Cargo did not identify both sale test executables")
    write_json(directory / "test-binaries.json", binaries)
    sources = [root / "Cargo.toml", root / "Cargo.lock", root / "upstream.toml"]
    for crate in ("ottd-core", "ottd-save", "ottd-sim", "ottd-cli"):
        base = root / "crates" / crate
        sources.append(base / "Cargo.toml")
        sources.extend(base.rglob("*.rs"))
        sources.extend(base.rglob("*.json"))
    for name in (
        "rust-toolchain",
        "rust-toolchain.toml",
        ".cargo/config",
        ".cargo/config.toml",
    ):
        path = root / name
        if path.is_file():
            sources.append(path)
    for folder, pattern in (
        ("reference", "*"),
        ("scripts", "*.py"),
        ("scripts", "*.cmake"),
        ("scripts", "*.sh"),
    ):
        sources.extend(path for path in (root / folder).glob(pattern) if path.is_file())
    sources.extend(
        [root / "scripts/reference.cfg", root / "scripts/sale-evidence-layout.json"]
    )
    hashes: dict[str, Json] = {
        str(path.relative_to(root)): digest(path) for path in sorted(sources)
    }
    write_json(
        directory / "source-provenance.json",
        {
            "source_tree": os.environ.get("OTTD_PURCHASE_SOURCE_TREE"),
            "files": hashes,
        },
    )


def binding(stderr: str, recorded: Json, cwd: Path) -> str:
    match recorded:
        case {"path": str() as executable, "sha256": str() as sha}:
            expected = Path(executable).resolve(strict=True)
        case _:
            raise WorldCheckError("Missing sale test executable identity")
    clean = re.sub(r"\x1b\[[0-?]*[ -/]*[@-~]", "", stderr)
    lines = [
        matched.group(1)
        for matched in re.finditer(
            r"^\s*Running (?:unittests src/lib\.rs|tests/native_sale\.rs) \((.+)\)\s*$",
            clean,
            re.MULTILINE,
        )
    ]
    if len(lines) != 1:
        raise WorldCheckError("Missing or ambiguous executed sale test path")
    actual = (cwd / lines[0]).resolve(strict=True)
    if actual != expected or digest(actual) != sha:
        raise WorldCheckError("Executed sale test binary differs from Cargo identity")
    return str(actual)


def verify(directory: Path, runtime_cases: tuple[str, ...]) -> None:
    root = Path(__file__).resolve().parents[1]
    before = read_json(directory / "before.json")
    if before != read_json(directory / "after.json"):
        raise WorldCheckError("Sale source/binary identity changed")
    match before:
        case {
            "source": dict() as sources,
            "binaries": {"native": str() as native_sha, "cli": str() as cli_sha},
        }:
            for name, sha in sources.items():
                if digest(root / name) != sha:
                    raise WorldCheckError("Sale source provenance changed")
        case _:
            raise WorldCheckError("Missing sale source provenance")
    provenance = read_json(directory / "results/provenance.json")
    match provenance:
        case {
            "native_sha256": str() as observed_native,
            "rust_sha256": str() as observed_cli,
        } if (observed_native, observed_cli) == (native_sha, cli_sha):
            pass
        case _:
            raise WorldCheckError("Sale executable provenance differs")
    binaries = read_json(directory / "test-binaries.json")
    match binaries:
        case {"native_sale": prep, "ottd_sim": runtime}:
            pass
        case _:
            raise WorldCheckError("Incomplete sale executable identities")
    invocations = [("cleanup-helper", prep), ("inputs-helper", prep)]
    invocations.extend((f"{name}/rust-command", runtime) for name in runtime_cases)
    observed: dict[str, Json] = {}
    for name, identity in invocations:
        path = binding(
            (directory / "results" / name / "stderr.log").read_text(), identity, root
        )
        observed[name] = {"path": path, "sha256": digest(Path(path))}
    for identity in (prep, runtime):
        match identity:
            case {"retained": str() as retained, "sha256": str() as sha}:
                if digest(Path(retained)) != sha:
                    raise WorldCheckError("Retained sale executable changed")
            case _:
                raise WorldCheckError("Retained sale executable missing")
    write_json(directory / "executed-test-binaries.json", observed)


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise WorldCheckError("Expected sale artifact directory and native oracle")
    prepare(Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve())
