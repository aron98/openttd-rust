# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = ["pytest"]
# ///
# Run: uv run --with pytest python -m pytest scripts/tests/rail_sources_test.py -q
"""Actual rail producer preflight rejects omitted or changed required source inputs."""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
import unittest
from functools import partial
from pathlib import Path

import pytest

from scripts.rail_sources import (
    MANIFEST,
    REQUIRED,
    capture,
    mapping,
    source_names,
    verify_copies,
)
from scripts.world_check_support import WorldCheckError, read_json, write_json

ROOT = Path(__file__).resolve().parents[2]


def invoke(producer: Path, output: Path) -> subprocess.CompletedProcess[str]:
    argv = ["bash", str(producer), str(output), "/missing/oracle"]
    result = subprocess.run(
        argv,
        capture_output=True,
        text=True,
        check=False,
        timeout=30,
    )
    retained = os.environ.get("RAIL_SOURCE_CONTROL_DIR")
    if retained is not None:
        path = Path(retained) / (output.name + ".json")
        path.parent.mkdir(parents=True, exist_ok=True)
        write_json(
            path,
            {
                "argv": list(argv),
                "returncode": result.returncode,
                "stdout": result.stdout,
                "stderr": result.stderr,
            },
        )
    return result


class RailSourcesTest(unittest.TestCase):
    def test_retained_byte_mode_and_membership_guards(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "capture"
            capture(ROOT, output)
            for name in REQUIRED:
                with self.subTest(source=name):
                    target = output / "source" / name
                    original, mode = target.read_bytes(), target.stat().st_mode
                    target.unlink()
                    with pytest.raises(WorldCheckError):
                        verify_copies(ROOT, output)
                    _ = target.write_bytes(original + b"\ncorrupted\n")
                    with pytest.raises(WorldCheckError):
                        verify_copies(ROOT, output)
                    _ = target.write_bytes(original)
                    target.chmod(mode ^ 0o100)
                    with pytest.raises(WorldCheckError):
                        verify_copies(ROOT, output)
                    target.chmod(mode)
            verify_copies(ROOT, output)

    def test_build_and_run_producers_reject_changed_mandatory_inputs(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            copied = base / "root"
            for name in (*source_names(ROOT), MANIFEST):
                destination = copied / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                _ = shutil.copy2(ROOT / name, destination)
            for producer in (
                "build-rail-restore-reference.sh",
                "check-rail-restore.sh",
                "build-rail-consist-reference.sh",
                "check-rail-consist.sh",
            ):
                for index, name in enumerate(REQUIRED):
                    target = copied / name
                    original, mode = target.read_bytes(), target.stat().st_mode
                    for corruption, mutate in (
                        ("bytes", partial(target.write_bytes, original + b"\n")),
                        ("missing", target.unlink),
                        ("mode", partial(target.chmod, mode ^ 0o100)),
                    ):
                        with self.subTest(
                            producer=producer, source=name, corruption=corruption
                        ):
                            _ = mutate()
                            output = base / f"{producer}-{index}-{corruption}"
                            result = invoke(copied / "scripts" / producer, output)
                            assert result.returncode != 0
                            if corruption != "missing":
                                assert (
                                    "Rail source bytes/mode mismatch" in result.stderr
                                )
                            assert not (output / "native-source").exists()
                            assert not (output / "engine-0").exists()
                            assert not (output / "0,27,29").exists()
                            _ = target.write_bytes(original)
                            target.chmod(mode)

    def test_missing_manifest_member_rejected_before_copy(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            copied = base / "root"
            capture(ROOT, base / "initial")
            _ = shutil.copytree(base / "initial/source", copied)
            inventory = mapping(read_json(ROOT / MANIFEST))
            sources = mapping(inventory["sources"])
            write_json(copied / MANIFEST, inventory)
            for name in REQUIRED:
                with self.subTest(source=name):
                    original = sources.pop(name)
                    write_json(copied / MANIFEST, inventory)
                    with pytest.raises(WorldCheckError):
                        capture(copied, base / "never-created")
                    assert not (base / "never-created").exists()
                    sources[name] = original


if __name__ == "__main__":
    _ = unittest.main()
