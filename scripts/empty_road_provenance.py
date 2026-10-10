from __future__ import annotations

import ast
import shutil
from pathlib import Path

from scripts.context_ci_support import sources
from scripts.empty_road_evidence import membership
from scripts.gameplay_foundations import digest
from scripts.grf_control_run import ControlRun
from scripts.language_ci_compare import compare, mapping
from scripts.owned_restore_sources import INCLUDE, LITERAL, compiled_inputs, regular
from scripts.script_vm_provenance import select_executable
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


def source_names(root: Path) -> tuple[str, ...]:
    names = set(compiled_inputs(root))
    names.update(
        (
            "upstream.toml",
            "rust-toolchain.toml",
            "scripts/reference.cfg",
            "scripts/setup-snapshot-reference.sh",
            "scripts/empty-road-fixtures.json",
            "fixtures/replay/clear-v362.sav",
        )
    )
    names.update(
        regular(root, path)
        for path in (root / "fixtures/road-tile").iterdir()
        if path.is_file()
    )
    pending = [root / "crates/ottd-sim/tests/road_tile_loop.rs"]
    while pending:
        path = pending.pop()
        names.add(regular(root, path))
        body = path.read_text()
        matches = list(LITERAL.finditer(body))
        if len(matches) != len(INCLUDE.findall(body)):
            raise WorldCheckError("Unresolved empty-road integration include")
        for item in matches:
            dependency = path.parent / item[1]
            names.add(regular(root, dependency))
            if item[0].startswith("include!"):
                pending.append(dependency)
    names.update(
        regular(root, path) for path in (root / "reference").iterdir() if path.is_file()
    )
    names.update(regular(root, path) for path in (root / "scripts").glob("*.cmake"))
    pending = [
        root / "scripts/check-empty-road.py",
        *root.glob("scripts/empty_road_*.py"),
    ]
    visited: set[str] = set()
    while pending:
        path = pending.pop()
        name = regular(root, path)
        if name in visited:
            continue
        visited.add(name)
        names.add(name)
        for item in ast.walk(ast.parse(path.read_text())):
            if (
                isinstance(item, ast.ImportFrom)
                and item.module
                and item.module.startswith("scripts.")
            ):
                pending.append(root / (item.module.replace(".", "/") + ".py"))
    return tuple(sorted(names))


def capture(root: Path, output: Path) -> Json:
    hashes: dict[str, Json] = {}
    for name in source_names(root):
        hashes[name] = digest(root / name)
        destination = output / "source" / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        _ = shutil.copy2(root / name, destination)
    write_json(output / "source-hashes.json", hashes)
    return hashes


def verify_sources(root: Path, hashes: Json) -> None:
    membership(sorted(mapping(hashes)), list(source_names(root)))
    sources(root, {"sources": hashes})


def build(job: ControlRun, target: Path) -> Json:
    result: dict[str, Json] = {}
    (job.output / "bin").mkdir()
    for key, name, kind, arguments in (
        (
            "runner",
            "road_tile_loop",
            "test",
            [
                "test",
                "--locked",
                "-p",
                "ottd-sim",
                "--test",
                "road_tile_loop",
                "--no-run",
            ],
        ),
        (
            "cli",
            "ottd",
            "bin",
            ["build", "--locked", "-p", "ottd-cli", "--bin", "ottd"],
        ),
    ):
        argv = [
            "env",
            "CARGO_INCREMENTAL=0",
            "CARGO_PROFILE_DEV_DEBUG=0",
            "CARGO_PROFILE_TEST_DEBUG=0",
            f"CARGO_TARGET_DIR={target}",
            "cargo",
            *arguments,
            "--message-format=json",
        ]
        process = job.run("cargo-" + key, argv)
        selected = select_executable(process.stdout, name, kind, test=key == "runner")
        retained = job.output / "bin" / key
        _ = shutil.copy2(selected, retained)
        retained.chmod(0o555)
        result[key] = {
            "original": str(selected),
            "retained": str(retained),
            "sha256": digest(retained),
        }
    write_json(job.output / "binaries.json", result)
    return result


def verify_binaries(output: Path, binaries: Json) -> None:
    for key, name, kind in (
        ("runner", "road_tile_loop", "test"),
        ("cli", "ottd", "bin"),
    ):
        selected = select_executable(
            (output / "logs" / ("cargo-" + key) / "stdout.log").read_text(),
            name,
            kind,
            test=key == "runner",
        )
        compare(str(selected), at(binaries, (key, "original")))
        for field in ("original", "retained"):
            value = at(binaries, (key, field))
            if not isinstance(value, str):
                raise WorldCheckError("Invalid empty-road executable identity")
            compare(digest(Path(value)), at(binaries, (key, "sha256")))


def admitted(root: Path, *, capture_only: bool) -> Json:
    layout = read_json(root / "scripts/empty-road-layout.json")
    if not capture_only:
        if at(layout, ("fresh_complete_run_verified",)) is not True:
            raise WorldCheckError("Empty-road layout is not admitted by a fresh run")
        verify_sources(root, at(layout, ("sources",)))
    return layout
