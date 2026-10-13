"""Bound original process invocation; every failure retains its actual artifacts."""

from __future__ import annotations

import json
import os
import shutil
from dataclasses import dataclass
from pathlib import Path

from .baseline import FileInput, bind
from .compare import checkpoint_words
from .events import validate
from .host import Limits, run, size
from .protocol import validate_request
from .value import DECODE, EvidenceError, Json, field, integer, require, string


def write(path: Path, value: Json) -> None:
    with path.open("x") as stream:
        json.dump(value, stream, indent=2)
        _ = stream.write("\n")


@dataclass(frozen=True, slots=True)
class Native:
    executable: FileInput
    config: FileInput
    closure: tuple[FileInput, ...]
    baseline_candidates: tuple[FileInput, ...]
    output: Path
    total_limit: int
    free_floor: int = 1073741824
    external_run_limit: int = 402653184

    def verify(self) -> None:
        require("native source closure missing", condition=bool(self.closure))
        for item in (
            self.executable,
            self.config,
            *self.closure,
            *self.baseline_candidates,
        ):
            item.verify()

    def run(self, name: str, source: Path, descriptor: Json, expected: str) -> Path:
        """Run an exact protocol with bound inputs, budgets and actual baseline."""
        require("descriptor object", condition=isinstance(descriptor, dict))
        if not isinstance(descriptor, dict):
            raise EvidenceError("descriptor object")
        validate_request(descriptor)
        require("native invocation name", condition=name.isidentifier())
        require(
            "absolute output directory",
            condition=self.output.is_absolute() and self.output.is_dir(),
        )
        self.verify()
        run_limit = min(
            integer(field(descriptor, "max_bytes")),
            self.external_run_limit,
            self.total_limit - size(self.output) - 1048576,
        )
        require("remaining proof budget", condition=run_limit > 0)
        require(
            "total proof reservation exhausted",
            condition=size(self.output) + run_limit <= self.total_limit,
        )
        require(
            "global free floor reservation",
            condition=shutil.disk_usage(self.output).free
            >= self.free_floor + run_limit,
        )
        input_file = FileInput.capture(source)
        directory = self.output / name
        directory.mkdir()
        protocol = directory / "protocol.json"
        config = directory / "openttd.cfg"
        _ = shutil.copyfile(self.config.path, config)
        write(protocol, descriptor)
        environment = {
            key: os.environ[key]
            for key in ("PATH", "HOME", "TMPDIR", "SYSTEMROOT")
            if key in os.environ
        }
        environment.update(
            {
                "LANG": "C",
                "TZ": "UTC",
                "OTTD_MOVEMENT_OBSERVE": "1",
                "OTTD_REPLAY_PATH": str(protocol),
                "OTTD_REPLAY_OUTPUT": str(directory),
            }
        )
        if field(descriptor, "mode") == "prepare":
            environment["OTTD_MOVEMENT_PREPARE"] = "1"
        argv = [
            str(self.executable.path),
            "-X",
            "-x",
            "-c",
            str(config),
            "-vnull:ticks=8",
            "-snull",
            "-mnull",
            "-g",
            str(source),
            "-d",
            "sl=2",
        ]
        write(
            directory / "invocation.json",
            {
                "argv": list(argv),
                "cwd": str(directory),
                "environment": dict(environment),
                "input": str(source),
                "input_sha256": input_file.digest,
                "native_sha256": self.executable.digest,
                "config_sha256": self.config.digest,
                "protocol_sha256": FileInput.capture(protocol).digest,
                "expected_outcome": expected,
                "source_inputs": [
                    {"path": str(item.path), "sha256": item.digest}
                    for item in self.closure
                ],
                "baseline_inventory": [
                    {
                        "path": str(item.path),
                        "sha256": item.digest,
                        "bytes": item.size,
                        "device": item.device,
                        "inode": item.inode,
                    }
                    for item in self.baseline_candidates
                ],
            },
        )
        limits = Limits(
            directory,
            self.output,
            run_limit,
            self.total_limit,
            self.free_floor,
            integer(field(descriptor, "max_seconds")),
        )
        process = run(argv, environment, limits)
        write(
            directory / "process.json",
            {
                "classification": process.classification,
                "exit_code": process.code,
                "seconds": process.seconds,
                "directory_bytes": process.directory_bytes,
                "proof_bytes": process.proof_bytes,
                "external_run_limit": run_limit,
                "external_proof_limit": self.total_limit,
                "free_floor": self.free_floor,
            },
        )
        self.verify()
        input_file.verify()
        require(
            "native process failed",
            condition=process.code == 0
            and process.classification == "native_process_exit",
        )
        require(
            "host storage budget",
            condition=size(directory) < run_limit
            and size(self.output) < self.total_limit,
        )
        selected = bind(
            DECODE((directory / "baseline.json").read_text()),
            self.baseline_candidates,
            self.executable.path.parent / "baseset",
        )
        write(
            directory / "baseline-inputs.json",
            [
                {
                    "ordinal": index,
                    "native_path": str(item.actual.path),
                    "inventory_path": str(item.inventory.path),
                    "before_sha256": item.inventory.digest,
                    "after_sha256": item.actual.digest,
                    "before_device": item.inventory.device,
                    "before_inode": item.inventory.inode,
                    "after_device": item.actual.device,
                    "after_inode": item.actual.inode,
                    "bytes": item.actual.size,
                }
                for index, item in enumerate(selected)
            ],
        )
        result = DECODE((directory / "results.json").read_text())
        require(
            "native outcome: " + string(field(result, "outcome")),
            condition=field(result, "outcome") == expected,
        )
        require(
            "native observer failure",
            condition=field(result, "observer_failure") == "none",
        )
        validate(directory, result)
        initial = checkpoint_words(directory, "initial")
        write(
            directory / "interactive-input.json",
            {
                "scope": "observed-process-input-no-interactive-consumption",
                "observed_initial_words": [initial[0], initial[1]],
                "initial_runtime_sha256": FileInput.capture(
                    directory / "initial.runtime.json"
                ).digest,
            },
        )
        return directory
