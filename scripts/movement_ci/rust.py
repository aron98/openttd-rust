"""Cargo-selected production CLI and public runtime harness invocation."""

from __future__ import annotations

import shutil
from dataclasses import dataclass
from pathlib import Path

from scripts.gameplay_foundations import require_test
from scripts.grf_control_run import ControlRun
from scripts.script_vm_provenance import select_executable

from .baseline import FileInput
from .native import write
from .protocol import cli_plan
from .value import Json, require


@dataclass(frozen=True, slots=True)
class Executable:
    original: FileInput
    retained: FileInput
    cargo_stdout: FileInput
    target: str
    kind: str

    def verify(self) -> None:
        for item in (self.original, self.retained, self.cargo_stdout):
            item.verify()
        selected = select_executable(
            self.cargo_stdout.path.read_text(),
            self.target,
            self.kind,
            test=self.kind == "test",
        )
        require(
            "Cargo selection differs",
            condition=selected == self.original.path
            and self.retained.digest == self.original.digest,
        )


@dataclass(frozen=True, slots=True)
class Rust:
    job: ControlRun
    cli: Executable
    harness: Executable

    def capture(self, name: str, source: Path, calls: int) -> Path:
        self.harness.verify()
        input_file = FileInput.capture(source)
        output = self.job.output / name
        request = self.job.output / (name + "-request.json")
        write(request, {"input": str(source), "output": str(output), "calls": calls})
        result = self.job.run(
            name,
            [
                str(self.harness.retained.path),
                "--ignored",
                "--exact",
                "capture_public_runtime",
                "--nocapture",
            ],
            {"OTTD_MOVEMENT_RUST_REQUEST": str(request)},
        )
        require_test(result.stdout, "capture_public_runtime")
        input_file.verify()
        self.harness.verify()
        return output

    def replay(self, name: str, source: Path, calls: int) -> Path:
        self.cli.verify()
        input_file = FileInput.capture(source)
        plan = self.job.output / (name + "-plan.json")
        output = self.job.output / name
        write(plan, cli_plan(calls))
        _ = self.job.run(
            name,
            [
                str(self.cli.retained.path),
                "replay-world",
                str(source),
                str(plan),
                str(output),
            ],
        )
        input_file.verify()
        self.cli.verify()
        return output


def build(job: ControlRun, target: Path) -> Rust:
    """Select only executables emitted by actual fresh Cargo JSON invocations."""
    require("fresh target directory required", condition=not target.exists())
    target.mkdir()
    (job.output / "bin").mkdir()
    selected: list[Executable] = []
    for name, kind, arguments in (
        ("ottd", "bin", ["build", "--locked", "-p", "ottd-cli", "--bin", "ottd"]),
        (
            "road_movement_ci",
            "test",
            [
                "test",
                "--locked",
                "-p",
                "ottd-sim",
                "--test",
                "road_movement_ci",
                "--no-run",
            ],
        ),
        (
            "road_movement",
            "test",
            [
                "test",
                "--locked",
                "-p",
                "ottd-sim",
                "--test",
                "road_movement",
                "--no-run",
            ],
        ),
    ):
        label = "cargo-" + (name if name == "road_movement" else kind)
        process = job.run(
            label,
            ["cargo", *arguments, "--message-format=json"],
            {
                "CARGO_INCREMENTAL": "0",
                "CARGO_BUILD_JOBS": "1",
                "CARGO_PROFILE_DEV_DEBUG": "0",
                "CARGO_PROFILE_TEST_DEBUG": "0",
                "CARGO_TARGET_DIR": str(target),
            },
        )
        original = select_executable(process.stdout, name, kind, test=kind == "test")
        retained = job.output / "bin" / name
        _ = shutil.copy2(original, retained)
        retained.chmod(0o555)
        selected.append(
            Executable(
                FileInput.capture(original),
                FileInput.capture(retained),
                FileInput.capture(job.output / "logs" / label / "stdout.log"),
                name,
                kind,
            )
        )
    rows: list[Json] = [
        {
            "target": item.target,
            "kind": item.kind,
            "original": str(item.original.path),
            "retained": str(item.retained.path),
            "sha256": item.retained.digest,
            "cargo_stdout_sha256": item.cargo_stdout.digest,
        }
        for item in selected
    ]
    write(job.output / "binaries.json", rows)
    for item in selected:
        item.verify()
    _ = job.run("rustc-version", ["rustc", "--version", "--verbose"])
    for selector in (
        "zero_calls_preserve_unsupported_side_and_all_supported_runtime",
        "unsupported_side_is_typed_and_does_not_publish",
        "offset_month_refuses_after_provisional_rng_and_motion",
        "phased_clock_exposes_calendar_before_economy_before_tick",
    ):
        result = job.run(
            selector,
            [str(selected[2].retained.path), "--exact", selector, "--nocapture"],
        )
        require_test(result.stdout, selector)
        selected[2].verify()
    return Rust(job, selected[0], selected[1])
