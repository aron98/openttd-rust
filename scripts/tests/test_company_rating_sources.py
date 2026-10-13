"""Real setup/replay producers reject omitted or tampered observer inputs."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from scripts.company_rating_sources import MANIFEST, REQUIRED, verify
from scripts.world_check_support import Json, read_json, write_json

ROOT = Path(__file__).resolve().parents[2]


def mutate(target: Path, mutation: str) -> tuple[bytes, int]:
    original, mode = target.read_bytes(), target.stat().st_mode
    if mutation == "digest":
        _ = target.write_bytes(original + b"\n")
    if mutation == "mode":
        target.chmod(mode ^ 0o100)
    if mutation == "raw-mode":
        target.chmod(mode ^ 0o010)
    if mutation == "physical":
        target.unlink()
    return original, mode


class CompanyRatingSourcesTest(unittest.TestCase):
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
                    str(root / "scripts/setup-company-rating-reference.sh"),
                    "/missing/native",
                ],
                [
                    "bash",
                    str(root / "scripts/setup-cargo-observer.sh"),
                    "/missing/native",
                ],
                [
                    "bash",
                    str(root / "scripts/setup-vehicle-year-reference.sh"),
                    "/missing/native",
                ],
                [
                    sys.executable,
                    "-m",
                    "scripts.company_rating_run",
                    "/missing/binary",
                    "/missing/save",
                    "/missing/actions",
                    "/missing/config",
                    str(root / "never-created"),
                    "off",
                ],
            )
            original_manifest = (root / MANIFEST).read_bytes()
            for producer_index, producer in enumerate(producers):
                for index, name in enumerate(REQUIRED):
                    for mutation in (
                        "omission",
                        "digest",
                        "mode",
                        "raw-mode",
                        "physical",
                    ):
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
                            original, mode = mutate(target, mutation)
                            write_json(root / MANIFEST, manifest)
                            result = subprocess.run(
                                producer,
                                cwd=root,
                                capture_output=True,
                                text=True,
                                check=False,
                                timeout=30,
                            )
                            expected = {
                                "omission": "inventory membership differs",
                                "physical": "required input missing/escaped",
                                "digest": "source bytes/mode mismatch",
                                "mode": "source bytes/mode mismatch",
                                "raw-mode": "source bytes/mode mismatch",
                            }[mutation]
                            retained = os.environ.get(
                                "COMPANY_RATING_SOURCE_CONTROL_DIR"
                            )
                            if retained:
                                output = Path(retained)
                                output.mkdir(parents=True, exist_ok=True)
                                filename = f"{producer_index}-{index}-{mutation}.json"
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
                            assert result.returncode != 0
                            assert (
                                expected in result.stderr
                                or (mutation == "physical"
                                and (
                                    str(target) in result.stderr
                                    or name.removesuffix(".py").replace("/", ".")
                                    in result.stderr
                                ))
                            )
                            assert not (root / "never-created").exists()

            verify(root)


if __name__ == "__main__":
    _ = unittest.main()
