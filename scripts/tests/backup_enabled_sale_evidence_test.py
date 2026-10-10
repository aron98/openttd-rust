from __future__ import annotations

import json
from copy import deepcopy
from pathlib import Path

import pytest

from scripts.backup_enabled_sale_evidence import critical_witnesses, validate
from scripts.backup_enabled_sale_provenance import build_argv, verify_all
from scripts.backup_sale_run import BackupSaleRun
from scripts.gameplay_foundations import digest
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    read_json,
    replace,
    write_json,
)


def test_old_bridge_and_enabled_observer_cannot_cross_identity(tmp_path: Path) -> None:
    old = BackupSaleRun(tmp_path, tmp_path, tmp_path / "native", {}, {})
    enabled = BackupSaleRun(
        tmp_path, tmp_path, tmp_path / "native", {}, {}, backup_enabled=True
    )
    assert old.observer_mode == "OTTD_BACKUP_SALE_OBSERVE"
    assert old.selector == "runtime::backup_sale_native::original_backup_sale_case"
    assert old.input_prefix == "BACKUP_SALE"
    assert enabled.observer_mode == "OTTD_BACKUP_ENABLED_SALE_OBSERVE"
    assert (
        enabled.selector
        == "runtime::backup_enabled_sale_native::original_backup_enabled_sale_case"
    )
    assert enabled.input_prefix == "BACKUP_ENABLED_SALE"
    with pytest.raises(WorldCheckError, match="distinct observer"):
        _ = validate(old, {})


@pytest.mark.parametrize(
    "changed", ["slot", "index", "user", "clone", "pool", "orders"]
)
def test_lifetime_witness_rejects_semantic_mutations(changed: str) -> None:
    native: Json = {
        "final": {
            "orders": {
                "backups": [
                    {
                        "pool_slot": 127,
                        "id": 127,
                        "user": 128,
                        "clone": None,
                        "orders": [{"wait_time": 31, "travel_time": 47}],
                    }
                ],
                "backup_pool": {"first_unused": 255, "occupied": [127]},
            }
        }
    }
    expected: Json = [
        {
            "path": ["final", "orders"],
            "value": deepcopy(at(native, ("final", "orders"))),
        }
    ]
    critical_witnesses(native, expected)
    fields: dict[str, tuple[str | int, ...]] = {
        "slot": ("final", "orders", "backups", 0, "pool_slot"),
        "index": ("final", "orders", "backups", 0, "id"),
        "user": ("final", "orders", "backups", 0, "user"),
        "clone": ("final", "orders", "backups", 0, "clone"),
        "pool": ("final", "orders", "backup_pool", "first_unused"),
        "orders": ("final", "orders", "backups", 0, "orders", 0, "wait_time"),
    }
    replace(native, fields[changed], 0)
    with pytest.raises(WorldCheckError, match="lifecycle witness"):
        critical_witnesses(native, expected)


@pytest.mark.parametrize("path", [[], [True], [None], ["missing"]])
def test_witness_path_must_be_nonempty_exact_json_path(path: Json) -> None:
    with pytest.raises(WorldCheckError):
        critical_witnesses({}, [{"path": path, "value": 0}])


def test_build_records_debug_only_overrides_without_optimization_change(
    tmp_path: Path,
) -> None:
    runner = build_argv(tmp_path, cli=False)
    assert runner == [
        "env",
        "CARGO_INCREMENTAL=0",
        "CARGO_PROFILE_DEV_DEBUG=0",
        "CARGO_PROFILE_TEST_DEBUG=0",
        f"CARGO_TARGET_DIR={tmp_path}",
        "cargo",
        "test",
        "--locked",
        "-p",
        "ottd-sim",
        "--lib",
        "--no-run",
        "--message-format=json",
    ]
    cli = build_argv(tmp_path, cli=True)
    assert cli[6:] == [
        "build",
        "--locked",
        "-p",
        "ottd-cli",
        "--bin",
        "ottd",
        "--message-format=json",
    ]


def build_evidence(root: Path) -> tuple[BackupSaleRun, Json]:
    output = root / "output"
    output.mkdir()
    _ = (root / "src.rs").write_text("source")
    (output / "source").mkdir()
    _ = (output / "source/src.rs").write_text("source")
    (output / "bin").mkdir()
    layout: Json = {"sources": {"src.rs": digest(root / "src.rs")}}
    write_json(
        output / "provenance.json",
        {"source_hashes": {"src.rs": digest(root / "src.rs")}},
    )
    target = root / "target"
    target.mkdir()
    write_json(
        output / "build-environment.json",
        {
            "CARGO_INCREMENTAL": "0",
            "CARGO_PROFILE_DEV_DEBUG": "0",
            "CARGO_PROFILE_TEST_DEBUG": "0",
            "CARGO_TARGET_DIR": str(target),
        },
    )
    binaries: dict[str, Json] = {}
    for name, target_name, kind, is_test in (
        ("runner", "ottd_sim", "lib", True),
        ("cli", "ottd", "bin", False),
    ):
        selected = target / name
        _ = selected.write_bytes(name.encode())
        _ = (output / "bin" / name).write_bytes(name.encode())
        log = output / "logs" / ("cargo-" + name)
        log.mkdir(parents=True)
        write_json(log / "argv.json", list(build_argv(target, cli=not is_test)))
        write_json(log / "process.json", {"returncode": 0, "expected": 0})
        _ = (log / "stdout.log").write_text(
            json.dumps(
                {
                    "reason": "compiler-artifact",
                    "executable": str(selected),
                    "target": {"name": target_name, "kind": [kind]},
                    "profile": {"test": is_test},
                }
            )
            + "\n"
        )
        binaries[name] = {
            "executable": str(output / "bin" / name),
            "original": str(selected),
            "sha256": digest(selected),
        }
    return BackupSaleRun(
        root, output, root / "native", binaries, {}, backup_enabled=True
    ), layout


@pytest.mark.parametrize(
    "corruption", ["selected", "argv", "source", "retained", "profile"]
)
def test_cargo_source_and_executable_binding_rejects_independent_corruptions(
    tmp_path: Path, corruption: str
) -> None:

    job, layout = build_evidence(tmp_path)
    verify_all(job, layout)
    match corruption:
        case "selected":
            replace(job.binaries, ("runner",), deepcopy(at(job.binaries, ("cli",))))
        case "argv":
            write_json(job.output / "logs/cargo-runner/argv.json", ["cargo", "test"])
        case "source":
            _ = (tmp_path / "src.rs").write_text("changed")
        case "retained":
            _ = (job.output / "bin/runner").write_bytes(b"changed")
        case "profile":
            path = job.output / "build-environment.json"
            environment = read_json(path)
            replace(environment, ("CARGO_PROFILE_TEST_DEBUG",), "1")
            write_json(path, environment)
        case _:
            pytest.fail("unknown corruption")
    with pytest.raises(WorldCheckError):
        verify_all(job, layout)
