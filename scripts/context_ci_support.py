# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through the context and occupancy CI drivers.
from __future__ import annotations

import json
import os
import shutil
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path
from typing import Final

from scripts.gameplay_foundations import digest, require_test
from scripts.grf_control_evidence import sequence, text
from scripts.grf_control_run import ControlRun
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    decode_json,
    read_json,
    write_json,
)

CONTEXT_TEST: Final = (
    "content::grf::load_context_tests::native::original_loader_context_matrix"
)
GUARD_TEST: Final = (
    "content::grf::load_context_tests::native::guards::original_context_host_guards"
)
OCCUPANCY_TEST: Final = "commands::road_depot::occupancy::native::native_ground_cutoff"


def exact(left: Json, right: Json) -> bool:
    return json.dumps(left, sort_keys=True, allow_nan=False) == json.dumps(
        right, sort_keys=True, allow_nan=False
    )


def refuse_subset() -> None:
    forbidden = {
        "OTTD_GRF_CONTROL_CASE",
        "OTTD_CONTEXT_CASE",
        "OTTD_OCCUPANCY_CASE",
        "OTTD_REPLAY_PATH",
    }
    if forbidden.intersection(os.environ):
        raise WorldCheckError(
            "Complete context/occupancy CI refuses subset or replay environment"
        )


def sources(root: Path, layout: Json) -> None:
    match at(layout, ("sources",)):
        case dict() as hashes if hashes:
            for name, expected in hashes.items():
                if (
                    not (root / name).resolve().is_relative_to(root)
                    or digest(root / name) != expected
                ):
                    raise WorldCheckError(f"Pinned witness source changed: {name}")
        case _:
            raise WorldCheckError("Missing pinned witness sources")


def build_lib(job: ControlRun) -> Path:
    result = job.run(
        "build",
        [
            "cargo",
            "test",
            "--locked",
            "-p",
            "ottd-sim",
            "--lib",
            "--no-run",
            "--message-format=json",
        ],
        {"CARGO_INCREMENTAL": "0"},
    )
    selected: list[Path] = []
    for line in result.stdout.splitlines():
        match decode_json(line):
            case {
                "reason": "compiler-artifact",
                "executable": str() as executable,
                "target": {"name": "ottd_sim", "kind": ["lib"]},
                "profile": {"test": True},
            }:
                selected.append(Path(executable).resolve(strict=True))
            case _:
                continue
    if len(selected) != 1:
        raise WorldCheckError("Expected exactly one actual lib test executable")
    destination = job.output / "bin"
    destination.mkdir()
    original = selected[0]
    retained = Path(shutil.copy2(original, destination / "ottd_sim"))
    retained.chmod(0o555)
    write_json(
        job.output / "test-binaries.json",
        {
            "ottd_sim": {
                "original": str(original),
                "retained": str(retained),
                "sha256": digest(retained),
                "kind": ["lib"],
                "profile_test": True,
            }
        },
    )
    return retained


@dataclass(frozen=True, slots=True)
class Selection:
    name: str
    ignored: bool = True
    actual: str | None = None


def run_exact(
    job: ControlRun,
    binary: Path,
    selection: Selection,
    variables: dict[str, str],
) -> None:
    argv = [
        str(binary),
        "--exact",
        selection.name if selection.actual is None else selection.actual,
        "--nocapture",
    ]
    if selection.ignored:
        argv.append("--ignored")
    result = job.run(selection.name, argv, variables)
    require_test(result.stdout, selection.name)


def zero(job: ControlRun, binary: Path) -> None:
    result = job.run(
        "zero-test",
        [str(binary), "--exact", "context_ci_nonexistent_test", "--ignored"],
    )
    try:
        require_test(result.stdout, CONTEXT_TEST)
    except WorldCheckError:
        if "0 passed; 0 failed; 0 ignored" not in result.stdout:
            raise WorldCheckError(
                "Zero-test guard did not execute zero tests"
            ) from None
    else:
        raise WorldCheckError("Zero tests were admitted")
    write_json(
        job.output / "zero-test-rejected.json",
        {"rejected": True, "actual_tests": 0, "binary_sha256": digest(binary)},
    )


def process(
    directory: Path, binary: Path, selector: str, *, ignored: bool = True
) -> None:
    require_test((directory / "stdout.log").read_text(), selector)
    argv = [text(value) for value in sequence(read_json(directory / "argv.json"))]
    if (
        not argv
        or Path(argv[0]).resolve() != binary.resolve()
        or argv.count(selector) != 1
        or "--exact" not in argv
        or (ignored and "--ignored" not in argv)
    ):
        raise WorldCheckError("Executed witness selector/executable differs")
    if at(read_json(directory / "process.json"), ("returncode",)) != 0:
        raise WorldCheckError("Witness process did not succeed")


def verify_identity(root: Path, output: Path, *, occupancy: bool = False) -> None:
    provenance = read_json(output / "provenance.json")
    key = "sources" if occupancy else "source_hashes"
    sources(root, {"sources": at(provenance, (key,))})
    oracle = Path(text(at(provenance, ("oracle" if occupancy else "native_binary",))))
    if digest(oracle) != at(
        provenance, ("oracle_sha256" if occupancy else "native_sha256",)
    ) or digest(oracle.parent / "replay-build.sha256") != at(
        provenance, ("stamp_sha256",)
    ):
        raise WorldCheckError("Witness native executable/stamp changed")
    match read_json(output / "test-binaries.json"):
        case dict() as rows if rows:
            expected = {"ottd_sim": (["lib"], True)}
            if occupancy:
                expected.update(
                    {"native_depot_build": (["test"], True), "ottd": (["bin"], False)}
                )
            if set(rows) != set(expected):
                raise WorldCheckError("Wrong executable identity set")
            for name, row in rows.items():
                kind, profile = expected[name]
                if (
                    at(row, ("kind",)) != kind
                    or at(row, ("profile_test",)) is not profile
                ):
                    raise WorldCheckError("Wrong Cargo executable kind/profile")
                for field in ("original", "retained"):
                    if digest(Path(text(at(row, (field,))))) != at(row, ("sha256",)):
                        raise WorldCheckError("Witness executable changed")
        case _:
            raise WorldCheckError("Missing actual executable identities")


def retain_native(output: Path, oracle: Path) -> None:
    destination = output / "native-executable"
    destination.mkdir()
    for name, source in (
        ("openttd", oracle),
        ("replay-build.sha256", oracle.parent / "replay-build.sha256"),
    ):
        _ = shutil.copy2(source, destination / name)
        (destination / name).chmod(0o555)
        if digest(destination / name) != digest(source):
            raise WorldCheckError("Retained native executable/stamp differs")


def invocation_fields(root: Path, path: Path) -> Mapping[str, str]:
    fields: dict[str, str] = {}
    for line in path.read_text().splitlines():
        key, separator, value = line.partition("=")
        if separator:
            if key == "SOURCE_SHA256":
                sha, name = value.split(" ", 1)
                if digest(root / name) != sha:
                    raise WorldCheckError("Native invocation source changed")
            else:
                fields[key] = value
    return fields


def native_bindings(root: Path, output: Path, oracle: Path) -> None:
    rows: dict[str, Json] = {}
    for path in sorted(output.rglob("invocation.txt")):
        if path.is_relative_to(output / "corruption"):
            continue
        fields = invocation_fields(root, path)
        if Path(fields["ORACLE"]).resolve() != oracle or fields[
            "ORACLE_SHA256"
        ] != digest(oracle):
            raise WorldCheckError("Native invocation executable binding differs")
        for key in ("INPUT", "REPLAY", "MANIFEST"):
            if key in fields and digest(Path(fields[key])) != fields[f"{key}_SHA256"]:
                raise WorldCheckError("Native invocation input changed")
        if "CONFIG_SHA256" in fields and fields["CONFIG_SHA256"] != digest(
            root / "scripts/reference.cfg"
        ):
            raise WorldCheckError("Native invocation config changed")
        row: dict[str, Json] = dict(fields)
        rows[str(path.relative_to(output))] = row
    if not rows:
        raise WorldCheckError("Missing actual native invocation bindings")
    write_json(output / "native-bindings.json", rows)
