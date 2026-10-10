# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through scripts/check-order-state.py.
from __future__ import annotations

import shutil
from pathlib import Path

from scripts.context_ci_support import exact, sources
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import text
from scripts.grf_control_run import ControlRun
from scripts.script_vm_provenance import select_executable
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


def snapshot(root: Path, output: Path, layout: Json) -> None:
    sources(root, layout)
    match at(layout, ("sources",)):
        case dict() as entries:
            for name in entries:
                path = output / "source" / name
                path.parent.mkdir(parents=True, exist_ok=True)
                _ = shutil.copy2(root / name, path)
        case _:
            raise WorldCheckError("Missing order source pins")
    _ = shutil.copy2(root / "scripts/order-state-layout.json", output / "layout.json")


def build(job: ControlRun, target: Path) -> Json:
    variables = {"CARGO_INCREMENTAL": "0", "CARGO_TARGET_DIR": str(target)}
    write_json(job.output / "build-environment.json", dict(variables))
    runner = job.run(
        "cargo-test",
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
        variables,
    )
    cli = job.run(
        "cargo-cli",
        [
            "cargo",
            "build",
            "--locked",
            "-p",
            "ottd-cli",
            "--bin",
            "ottd",
            "--message-format=json",
        ],
        variables,
    )
    paths = (
        ("runner", select_executable(runner.stdout, "ottd_sim", "lib", test=True)),
        ("cli", select_executable(cli.stdout, "ottd", "bin", test=False)),
    )
    (job.output / "bin").mkdir()
    result: dict[str, Json] = {}
    for name, source in paths:
        target = job.output / "bin" / name
        _ = shutil.copy2(source, target)
        target.chmod(0o555)
        result[name] = {
            "executable": str(target),
            "original": str(source),
            "sha256": digest(target),
        }
    result["case_test"] = "runtime::order_state::native::original_order_state_case"
    result["airport_test"] = "runtime::order_state::native::original_airport_geometry"
    write_json(job.output / "binaries.json", result)
    return result


def verify(root: Path, output: Path, layout: Json, manifest: Json) -> None:
    sources(root, layout)
    for name in ("runner", "cli"):
        if digest(Path(text(at(manifest, (name, "executable"))))) != at(
            manifest, (name, "sha256")
        ):
            raise WorldCheckError("Order executable changed")
    match at(layout, ("sources",)):
        case dict() as entries:
            for name, sha in entries.items():
                if digest(output / "source" / name) != sha:
                    raise WorldCheckError("Retained order source changed")
        case _:
            raise WorldCheckError("Missing retained order source identity")


def validate_native_guard(root: Path, output: Path, oracle: Path) -> None:
    directory = output / "controls/native"
    binary = directory / "openttd"
    metadata = read_json(directory / "mutation.json")
    before, after = oracle.read_bytes(), binary.read_bytes()
    if (
        before[:-1] != after[:-1]
        or before[-1] ^ after[-1] != 1
        or not exact(
            metadata,
            {
                "before_sha256": digest(oracle),
                "after_sha256": digest(binary),
                "byte_offset": len(after) - 1,
                "xor": 1,
            },
        )
    ):
        raise WorldCheckError("Native admission mutation was not one actual byte")
    argv: list[Json] = [
        "cmake",
        f"-DORACLE={binary}",
        "-P",
        str(root / "scripts/check-replay-build.cmake"),
    ]
    for phase, expected in (("before", 0), ("rejected", 1)):
        if not exact(read_json(directory / phase / "argv.json"), argv) or not exact(
            read_json(directory / phase / "process.json"),
            {"returncode": expected, "expected": expected},
        ):
            raise WorldCheckError("Native freshness admission process differs")
    if (
        "Stale native replay binary/source"
        not in (directory / "rejected/stderr.log").read_text()
    ):
        raise WorldCheckError("Native stale executable rejection missing")
