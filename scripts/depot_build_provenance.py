# SPDX-License-Identifier: GPL-2.0-only
from __future__ import annotations

import os
import re
import shutil
from dataclasses import dataclass
from pathlib import Path

from scripts.gameplay_foundations import FoundationRun, digest, require_test
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    decode_json,
    read_json,
    write_json,
)

RUNTIME_TEST = "runtime::purchase_native::run_native_purchase_sequence"


@dataclass(frozen=True, slots=True)
class Binaries:
    runtime: Path
    prep: Path
    cli: Path


def build(run: FoundationRun) -> Binaries:
    selected: dict[str, Json] = {}
    retained = run.output / "bin"
    retained.mkdir()
    for label, command, targets in (
        (
            "build-tests",
            [
                "cargo",
                "test",
                "--locked",
                "-p",
                "ottd-sim",
                "--lib",
                "--test",
                "native_depot_build",
                "--no-run",
                "--message-format=json",
            ],
            {"ottd_sim": (["lib"], True), "native_depot_build": (["test"], True)},
        ),
        (
            "build-cli",
            ["cargo", "build", "--locked", "-p", "ottd-cli", "--message-format=json"],
            {"ottd": (["bin"], False)},
        ),
    ):
        result = run.run(label, command)
        for line in result.stdout.splitlines():
            match decode_json(line):
                case {
                    "reason": "compiler-artifact",
                    "executable": str() as executable,
                    "target": {"name": str() as name, "kind": kind},
                    "profile": {"test": test},
                } if name in targets:
                    if (kind, test) != targets[name] or name in selected:
                        raise WorldCheckError("Wrong or ambiguous depot Cargo artifact")
                    source = Path(executable).resolve(strict=True)
                    target = Path(shutil.copy2(source, retained / name))
                    selected[name] = {
                        "original": str(source),
                        "retained": str(target),
                        "sha256": digest(source),
                        "kind": kind,
                        "profile_test": test,
                    }
                case _:
                    continue
    if set(selected) != {"ottd_sim", "native_depot_build", "ottd"}:
        raise WorldCheckError("Missing depot Cargo executable")
    write_json(run.output / "test-binaries.json", selected)
    return Binaries(
        retained / "ottd_sim", retained / "native_depot_build", retained / "ottd"
    )


def snapshot(root: Path, output: Path, oracle: Path) -> None:
    paths = {
        root / name
        for name in (
            "Cargo.toml",
            "Cargo.lock",
            "upstream.toml",
            "scripts/reference.cfg",
            "fixtures/replay/clear-v362.sav",
        )
    }
    for crate in (root / "crates").iterdir():
        if not crate.is_dir():
            continue
        paths.add(crate / "Cargo.toml")
        for directory in ("src", "tests"):
            paths.update(
                path
                for path in (crate / directory).rglob("*")
                if path.is_file() and path.suffix in {".rs", ".json"}
            )
    for directory, suffixes in (
        ("scripts", {".py", ".json", ".cmake", ".sh"}),
        ("reference", {".hpp", ".patch"}),
    ):
        paths.update(
            path
            for path in (root / directory).rglob("*")
            if path.is_file() and path.suffix in suffixes
        )
    paths.update(
        root / name
        for name in (
            "rust-toolchain",
            "rust-toolchain.toml",
            ".cargo/config",
            ".cargo/config.toml",
        )
        if (root / name).is_file()
    )
    write_json(
        output / "provenance.json",
        {
            "source_tree": os.environ.get("OTTD_DEPOT_BUILD_SOURCE_TREE"),
            "sources": {
                str(path.relative_to(root)): digest(path) for path in sorted(paths)
            },
            "oracle": str(oracle),
            "oracle_sha256": digest(oracle),
            "stamp_sha256": digest(oracle.parent / "replay-build.sha256"),
        },
    )


def input_snapshot(output: Path) -> None:
    roots = ("inputs", "fleet-input", "original-fleet", "original-depots")
    write_json(
        output / "inputs-before.json",
        {
            str(path.relative_to(output)): digest(path)
            for name in roots
            for path in sorted((output / name).rglob("*"))
            if path.is_file()
        },
    )


def binding(stderr: str, identity: Json, root: Path) -> str:
    match identity:
        case {
            "original": str() as original,
            "retained": str() as retained,
            "sha256": str() as sha,
            "kind": ["lib"],
            "profile_test": True,
        }:
            pass
        case _:
            raise WorldCheckError("Wrong depot runtime executable identity")
    clean = re.sub(r"\x1b\[[0-?]*[ -/]*[@-~]", "", stderr)
    paths = [
        match.group(1)
        for match in re.finditer(
            r"^\s*Running unittests src/lib\.rs \((.+)\)\s*$", clean, re.MULTILINE
        )
    ]
    if len(paths) != 1 or (root / paths[0]).resolve(strict=True) != Path(original):
        raise WorldCheckError("Missing or wrong executed depot runtime binary")
    if digest(Path(original)) != sha or digest(Path(retained)) != sha:
        raise WorldCheckError("Executed depot binary changed")
    return original


def verify(root: Path, output: Path, cases: tuple[str, ...]) -> None:
    provenance = read_json(output / "provenance.json")
    hashes = at(provenance, ("sources",))
    inputs = read_json(output / "inputs-before.json")
    for base, values in ((root, hashes), (output, inputs)):
        match values:
            case dict() if values:
                for name, sha in values.items():
                    if digest(base / name) != sha:
                        raise WorldCheckError(f"Depot source/input changed: {name}")
            case _:
                raise WorldCheckError("Missing depot source/input hashes")
    match at(provenance, ("oracle",)):
        case str() as oracle:
            if digest(Path(oracle)) != at(provenance, ("oracle_sha256",)) or digest(
                Path(oracle).parent / "replay-build.sha256"
            ) != at(provenance, ("stamp_sha256",)):
                raise WorldCheckError("Native depot binary/stamp changed")
        case _:
            raise WorldCheckError("Missing native depot identity")
    binaries = read_json(output / "test-binaries.json")
    for name, kind, test in (
        ("ottd_sim", ["lib"], True),
        ("native_depot_build", ["test"], True),
        ("ottd", ["bin"], False),
    ):
        row = at(binaries, (name,))
        if at(row, ("kind",)) != kind or at(row, ("profile_test",)) is not test:
            raise WorldCheckError("Wrong depot Cargo artifact type")
        for key in ("original", "retained"):
            match at(row, (key,)):
                case str() as filename if digest(Path(filename)) == at(
                    row, ("sha256",)
                ):
                    pass
                case _:
                    raise WorldCheckError("Depot executable hash changed")
    observed: dict[str, Json] = {}
    for name in (*cases, "continuation/prefix", "continuation/suffix"):
        directory = output / "results" / name / "rust-command"
        require_test((directory / "stdout.log").read_text(), RUNTIME_TEST)
        path = binding(
            (directory / "stderr.log").read_text(), at(binaries, ("ottd_sim",)), root
        )
        observed[name] = {"path": path, "sha256": digest(Path(path))}
    for name in ("prepare_fleet_source", "prepare_depot_inputs"):
        directory = output / "logs" / name
        require_test((directory / "stdout.log").read_text(), name)
        expected: list[Json] = [
            at(binaries, ("native_depot_build", "retained")),
            "--ignored",
            "--exact",
            name,
            "--nocapture",
        ]
        if read_json(directory / "argv.json") != expected or read_json(
            directory / "process.json"
        ) != {"returncode": 0}:
            raise WorldCheckError("Depot fixture executable binding differs")
        observed[name] = at(binaries, ("native_depot_build",))
    write_json(output / "executed-test-binaries.json", observed)
    write_json(
        output / "provenance-verified.json",
        {
            "sources": hashes,
            "inputs": inputs,
            "binaries": binaries,
            "native": provenance,
            "runtime_invocations": len(cases) + 2,
        },
    )
