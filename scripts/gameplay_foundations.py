from __future__ import annotations

import hashlib
import os
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

from scripts.world_check_support import Json, WorldCheckError, decode_json, write_json


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fresh_directory(root: Path, requested: str | None) -> Path:
    parent = root / ".artifacts"
    parent.mkdir(exist_ok=True)
    if requested is None:
        return Path(tempfile.mkdtemp(prefix="gameplay-foundations-", dir=parent))
    output = Path(requested).resolve()
    if output.parent != parent.resolve():
        raise WorldCheckError(
            "Foundation artifacts must be a direct child of .artifacts"
        )
    output.mkdir(exist_ok=False)
    return output


def require_files(root: Path, names: list[str]) -> None:
    for name in names:
        path = root / name
        if (
            not path.resolve().is_relative_to(root.resolve())
            or not path.is_file()
            or path.stat().st_size == 0
        ):
            raise WorldCheckError(
                f"Missing, empty or escaped foundation evidence: {name}"
            )


def require_test(output: str, name: str) -> None:
    if (
        f"test {name} ... ok" not in output
        or "test result: ok. 1 passed; 0 failed; 0 ignored;" not in output
    ):
        raise WorldCheckError(
            f"Exact ignored test was not executed successfully: {name}"
        )


@dataclass(frozen=True)
class FoundationRun:
    root: Path
    output: Path
    oracle: Path

    def run(
        self, name: str, argv: list[str], variables: dict[str, str] | None = None
    ) -> subprocess.CompletedProcess[str]:
        directory = self.output / "logs" / name
        directory.mkdir(parents=True)
        write_json(directory / "argv.json", list(argv))
        environment = os.environ.copy()
        environment.update(variables or {})
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
        write_json(directory / "process.json", {"returncode": result.returncode})
        if result.returncode != 0:
            raise WorldCheckError(f"Foundation process failed: {name}; see {directory}")
        return result

    def build(self) -> dict[str, Path]:
        binaries: dict[str, Path] = {}
        packages = {
            "ottd-sim": [
                "native_content",
                "native_runtime_pools",
                "native_runtime_road",
                "native_grf_scan",
                "terrain",
            ],
            "ottd-core": ["terrain_oracle"],
        }
        for package, names in packages.items():
            argv = [
                "cargo",
                "test",
                "--locked",
                "-p",
                package,
                "--no-run",
                "--message-format=json",
            ]
            for name in names:
                argv.extend(["--test", name])
            result = self.run(f"build-{package}", argv)
            for line in result.stdout.splitlines():
                entry = decode_json(line)
                match entry:
                    case {
                        "reason": "compiler-artifact",
                        "executable": str() as executable,
                        "target": {"name": str() as name},
                    } if name in names:
                        binaries[name] = Path(executable).resolve()
                    case _:
                        continue
        if set(binaries) != {name for names in packages.values() for name in names}:
            raise WorldCheckError(
                "Cargo did not produce every foundation test executable"
            )
        write_json(
            self.output / "test-binaries.json",
            {
                name: {"path": str(path), "sha256": digest(path)}
                for name, path in binaries.items()
            },
        )
        return binaries

    def test(self, binary: Path, name: str, variables: dict[str, str]) -> None:
        result = self.run(
            name, [str(binary), "--ignored", "--exact", name, "--nocapture"], variables
        )
        require_test(result.stdout, name)

    def provenance(self) -> None:
        paths = {
            self.root / "Cargo.toml",
            self.root / "Cargo.lock",
            self.root / "upstream.toml",
            self.root / "scripts/reference.cfg",
        }
        for package in ["ottd-core", "ottd-save", "ottd-sim"]:
            base = self.root / "crates" / package
            paths.add(base / "Cargo.toml")
            for folder in ["src", "tests"]:
                paths.update(
                    path
                    for path in (base / folder).rglob("*")
                    if path.is_file() and path.suffix in {".rs", ".json"}
                )
        paths.update((self.root / "reference").glob("*.hpp"))
        paths.update((self.root / "reference").glob("*.patch"))
        paths.update((self.root / "scripts").glob("*.cmake"))
        paths.update(
            self.root / path
            for path in [
                "scripts/check-gameplay-foundations.py",
                "scripts/gameplay_foundations.py",
                "scripts/check-terrain.sh",
                "fixtures/generated-v362.sav",
                "fixtures/replay/clear-v362.sav",
                "fixtures/content/contract-speed.grf",
            ]
        )
        source_hashes: dict[str, Json] = {
            str(path.relative_to(self.root)): digest(path) for path in sorted(paths)
        }
        write_json(
            self.output / "provenance.json",
            {
                "upstream_commit": "14ec60f248547d4d062a1160f0fc26d742319888",
                "native_binary": str(self.oracle),
                "archive_tree": os.environ.get("OTTD_FOUNDATIONS_SOURCE_TREE"),
                "native_sha256": digest(self.oracle),
                "native_build_stamp_sha256": digest(
                    self.oracle.parent / "replay-build.sha256"
                ),
                "source_hashes": source_hashes,
            },
        )

    def finish(self) -> None:
        require_files(
            self.output,
            [
                "content/summary.txt",
                "terrain/summary.txt",
                "pools/native/allocation.json",
                "pools/rust/comparison.log",
                "road/summary.txt",
                "grf/summary.txt",
                "provenance.json",
                "test-binaries.json",
            ],
        )
        coverage = {
            "content": "PASS 52 native catalog cases",
            "road": "PASS 24 cases; 870 native road cache rows",
            "grf": "PASS 64 original FILESCAN cases",
        }
        for family, prefix in coverage.items():
            if (
                not (self.output / family / "summary.txt")
                .read_text()
                .startswith(prefix)
            ):
                raise WorldCheckError(f"Foundation coverage changed: {family}")
        paths = sorted(
            path
            for path in self.output.rglob("*")
            if path.is_file() and path.stat().st_size > 0
        )
        names = [str(path.relative_to(self.output)) for path in paths]
        require_files(self.output, names)
        write_json(
            self.output / "manifest.json",
            {name: digest(self.output / name) for name in names},
        )
        write_json(
            self.output / "summary.json",
            {
                "passed": True,
                "families": ["content", "terrain", "pools", "road", "grf"],
                "nonempty_artifacts": len(names),
            },
        )
