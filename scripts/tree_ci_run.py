from __future__ import annotations

import os
import shutil
from dataclasses import dataclass
from pathlib import Path

from scripts.context_ci_support import Selection, build_lib, run_exact
from scripts.gameplay_foundations import digest, log_name, require_test
from scripts.grf_control_run import ControlRun
from scripts.script_vm_provenance import select_executable
from scripts.world_check_support import WorldCheckError, write_json


@dataclass(frozen=True)
class TreeRun:
    job: ControlRun
    target: Path

    def build(self) -> tuple[Path, Path]:
        variables = {
            "CARGO_TARGET_DIR": str(self.target),
            "CARGO_INCREMENTAL": "0",
            "CARGO_BUILD_JOBS": "1",
            "CARGO_PROFILE_DEV_DEBUG": "0",
            "CARGO_PROFILE_TEST_DEBUG": "0",
        }
        prior = {key: os.environ.get(key) for key in variables}
        os.environ.update(variables)
        try:
            lib = build_lib(self.job)
            result = self.job.run(
                "build-cli",
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
        finally:
            for key, value in prior.items():
                if value is None:
                    _ = os.environ.pop(key, None)
                else:
                    os.environ[key] = value
        original = select_executable(result.stdout, "ottd", "bin", test=False)
        cli = Path(shutil.copy2(original, self.job.output / "bin/ottd"))
        write_json(
            self.job.output / "cli-binary.json",
            {"original": str(original), "retained": str(cli), "sha256": digest(cli)},
        )
        write_json(self.job.output / "build-environment.json", {**variables})
        return (lib, cli)

    def test(self, lib: Path, name: str, variables: dict[str, str]) -> None:
        run_exact(self.job, lib, Selection(name), variables)
        write_json(
            self.job.output / "logs" / log_name(name) / "tree-environment.json",
            {**variables},
        )

    def native(
        self, identity: str, source: Path, protocol: Path, *, observe: bool
    ) -> Path:
        group, name = identity.split("/", 1)
        directory = self.job.output / group / "runs" / name
        variables = {"OTTD_TREE_RATING_OBSERVE": "1"} if observe else {}
        _ = self.job.run(
            "native/" + identity,
            [
                "cmake",
                f"-DORACLE={self.job.oracle}",
                f"-DRUN_DIR={directory}",
                f"-DINPUT={source}",
                f"-DREPLAY={protocol}",
                f"-DCONFIG={self.job.root / 'scripts/reference.cfg'}",
                "-P",
                str(self.job.root / "scripts/check-replay-native.cmake"),
            ],
            variables,
        )
        return directory

    def zero(self, lib: Path) -> None:
        result = self.job.run(
            "zero-test",
            [str(lib), "--exact", "tree_ci_nonexistent_selector", "--ignored"],
        )
        if "0 passed; 0 failed; 0 ignored" not in result.stdout:
            raise WorldCheckError(
                "Zero-test control did not actually execute zero tests"
            )
        try:
            require_test(
                result.stdout,
                "commands::terrain_run::corpus::native_corpus_complete_receipts_worlds_derived_and_traces",
            )
        except WorldCheckError:
            write_json(
                self.job.output / "zero-test-rejected.json",
                {"actual_tests": 0, "rejected": True, "binary_sha256": digest(lib)},
            )
        else:
            raise WorldCheckError("Zero tests were accepted")
