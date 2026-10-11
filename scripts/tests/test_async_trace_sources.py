"""Real setup/replay producers reject omitted or tampered observer inputs."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from scripts.async_trace_sources import MANIFEST, REQUIRED, verify
from scripts.world_check_support import Json, read_json, write_json

ROOT = Path(__file__).resolve().parents[2]


class AsyncTraceSourcesTest(unittest.TestCase):
    def test_complete_current_sources(self) -> None:
        verify(ROOT)

    def test_actual_producers_reject_every_required_input(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "root"
            for folder in ("scripts", "reference"):
                _ = shutil.copytree(ROOT / folder, root / folder)
            producers = (
                [
                    "bash",
                    str(root / "scripts/setup-async-trace-reference.sh"),
                    "/missing/native",
                ],
                [
                    sys.executable,
                    "-m",
                    "scripts.async_trace_run",
                    "/missing/binary",
                    "/missing/save",
                    "/missing/actions",
                    "/missing/config",
                    str(root / "never-created"),
                    "off",
                    "off",
                ],
            )
            original_manifest = (root / MANIFEST).read_bytes()
            for producer in producers:
                for index, name in enumerate(REQUIRED[:12]):
                    for mutation in ("omission", "digest", "mode"):
                        with self.subTest(
                            producer=producer[0], source=name, mutation=mutation
                        ):
                            manifest = read_json(root / MANIFEST)
                            assert isinstance(manifest, dict)
                            key = "modes" if mutation == "mode" else "sources"
                            entries = manifest[key]
                            assert isinstance(entries, dict)
                            if mutation == "omission":
                                del entries[name]
                            target = root / name
                            original, mode = target.read_bytes(), target.stat().st_mode
                            if mutation == "digest":
                                _ = target.write_bytes(original + b"\n")
                            if mutation == "mode":
                                target.chmod(mode ^ 0o100)
                            write_json(root / MANIFEST, manifest)
                            result = subprocess.run(
                                producer,
                                cwd=root,
                                capture_output=True,
                                text=True,
                                check=False,
                                timeout=30,
                            )
                            expected = (
                                "inventory membership differs"
                                if mutation == "omission"
                                else "source bytes/mode mismatch"
                            )
                            assert result.returncode != 0
                            assert expected in result.stderr
                            assert not (root / "never-created").exists()
                            retained = os.environ.get("ASYNC_SOURCE_CONTROL_DIR")
                            if retained:
                                output = Path(retained)
                                output.mkdir(parents=True, exist_ok=True)
                                filename = (
                                    f"{Path(producer[0]).name}-{index}-{mutation}.json"
                                )
                                arguments: list[Json] = list(producer)
                                write_json(
                                    output / filename,
                                    {
                                        "argv": arguments,
                                        "returncode": result.returncode,
                                        "stdout": result.stdout,
                                        "stderr": result.stderr,
                                        "source": name,
                                    },
                                )
                            _ = target.write_bytes(original)
                            target.chmod(mode)
                            _ = (root / MANIFEST).write_bytes(original_manifest)
            verify(root)


if __name__ == "__main__":
    _ = unittest.main()
