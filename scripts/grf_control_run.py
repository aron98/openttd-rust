from __future__ import annotations

import os
import shutil
import subprocess
from collections.abc import Mapping
from dataclasses import dataclass
from pathlib import Path

from scripts.gameplay_foundations import digest, require_test
from scripts.world_check_support import Json, WorldCheckError, decode_json, write_json


@dataclass(frozen=True, slots=True)
class ControlRun:
    root: Path
    output: Path
    oracle: Path

    def run(
        self,
        name: str,
        argv: list[str],
        variables: Mapping[str, str] | None = None,
        *,
        expected: int = 0,
    ) -> subprocess.CompletedProcess[str]:
        directory = self.output / "logs" / name
        directory.mkdir(parents=True, exist_ok=False)
        environment = os.environ.copy()
        environment.update(variables or {})
        write_json(directory / "argv.json", list(argv))
        write_json(
            directory / "environment.json",
            {
                key: value
                for key, value in environment.items()
                if key.startswith(("OTTD_", "CARGO_TARGET_DIR"))
            },
        )
        result = subprocess.run(
            argv,
            cwd=self.root,
            env=environment,
            capture_output=True,
            text=True,
            timeout=1200,
            check=False,
        )
        _ = (directory / "stdout.log").write_text(result.stdout)
        _ = (directory / "stderr.log").write_text(result.stderr)
        write_json(
            directory / "process.json",
            {"returncode": result.returncode, "expected": expected},
        )
        if result.returncode != expected:
            raise WorldCheckError(f"Loader process failed: {name}; see {directory}")
        return result

    def build(self) -> dict[str, Path]:
        names = {"native_grf_control", "grf_load_control"}
        result = self.run(
            "build",
            [
                "cargo",
                "test",
                "--locked",
                "-p",
                "ottd-sim",
                "--no-run",
                "--message-format=json",
                "--test",
                "native_grf_control",
                "--test",
                "grf_load_control",
            ],
            {"CARGO_TARGET_DIR": str(self.output / "target")},
        )
        binaries: dict[str, Path] = {}
        metadata: dict[str, Json] = {}
        destination = self.output / "bin"
        destination.mkdir()
        for line in result.stdout.splitlines():
            match decode_json(line):
                case {
                    "reason": "compiler-artifact",
                    "executable": str() as executable,
                    "target": {"name": str() as name, "kind": ["test"]},
                    "profile": {"test": True},
                } if name in names:
                    if name in binaries:
                        raise WorldCheckError("Duplicate Cargo test executable")
                    path = Path(shutil.copy2(executable, destination / name))
                    path.chmod(0o555)
                    binaries[name] = path
                    metadata[name] = {
                        "path": str(path),
                        "original_path": executable,
                        "kind": ["test"],
                        "profile_test": True,
                        "sha256": digest(path),
                    }
                case _:
                    continue
        if set(binaries) != names:
            raise WorldCheckError(
                "Cargo did not produce both actual loader test executables"
            )
        write_json(self.output / "test-binaries.json", metadata)
        return binaries

    def test(self, binary: Path, name: str, *, ignored: bool = False) -> None:
        argv = [str(binary), "--exact", name, "--nocapture"]
        if ignored:
            argv.append("--ignored")
        result = self.run(
            name,
            argv,
            {
                "OTTD_GRF_CONTROL_ORACLE": str(self.oracle),
                "OTTD_GRF_CONTROL_DIR": str(self.output / "results"),
            },
        )
        require_test(result.stdout, name)

    def provenance(self) -> None:
        paths = {
            self.root / name
            for name in (
                "Cargo.toml",
                "Cargo.lock",
                "upstream.toml",
                "rust-toolchain.toml",
                "fixtures/replay/clear-v362.sav",
                "scripts/reference.cfg",
                "compatibility/contract.json",
                ".github/workflows/ci.yml",
            )
        }
        for folder in ("crates", "reference", "scripts"):
            paths.update(
                path
                for path in (self.root / folder).rglob("*")
                if path.is_file()
                and path.suffix
                in {".rs", ".toml", ".json", ".py", ".sh", ".cmake", ".hpp", ".patch"}
            )
        write_json(
            self.output / "provenance.json",
            {
                "native_binary": str(self.oracle),
                "native_sha256": digest(self.oracle),
                "upstream_commit": "14ec60f248547d4d062a1160f0fc26d742319888",
                "source_hashes": {
                    str(path.relative_to(self.root)): digest(path)
                    for path in sorted(paths)
                    if path.exists()
                },
                "stamp_sha256": digest(self.oracle.parent / "replay-build.sha256"),
            },
        )
