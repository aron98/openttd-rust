# SPDX-License-Identifier: GPL-2.0-only
"""External Rust complete-loop comparisons and opposite-origin save continuations."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from scripts.gameplay_foundations import digest, require_test
from scripts.grf_control_run import ControlRun
from scripts.house_native import HouseNative
from scripts.house_runtime_recipe import checkpoints
from scripts.tree_growth_run import compare
from scripts.world_check_support import write_json

ROOT = Path(__file__).resolve().parents[1]
TEST = "native_house_lifecycle_matches_complete_runtime"


@dataclass(frozen=True, slots=True)
class HouseProof:
    native: HouseNative
    lifecycle: Path
    controls: Path

    def run(self, native: Path, output: Path) -> None:
        runner = ControlRun(
            ROOT, output.parent / (output.name + "-logs"), self.lifecycle
        )
        result = runner.run(
            "lifecycle",
            [str(self.lifecycle), "--ignored", "--exact", TEST, "--nocapture"],
            {
                "OTTD_HOUSE_NATIVE_DIR": str(native),
                "OTTD_HOUSE_RUST_OUTPUT": str(output),
            },
        )
        require_test(result.stdout, TEST)
        write_json(
            output / "bindings.json",
            {
                "binary": str(self.lifecycle),
                "binary_sha256": digest(self.lifecycle),
                "native": str(native),
                "input_sha256": digest(native / "initial.sav"),
                "protocol_sha256": digest(native / "protocol.json"),
            },
        )

    def continuation(self, corpus: Path, output: Path) -> None:
        for name, source in [
            ("native-origin", corpus / "original-6144" / "tick_2048.sav"),
            ("rust-origin", output / "original-6144" / "tick_2048.sav"),
        ]:
            native = output / (name + "-native")
            _ = self.native.run(native, checkpoints(4096), source=source)
            self.run(native, output / (name + "-rust"))
            for view in ("world", "derived", "houses"):
                compare(
                    corpus / "original-6144" / f"final.{view}.json",
                    native / f"final.{view}.json",
                    output / f"{name}-{view}.json",
                )

    def controls_run(self, ready: Path, corpus: Path, output: Path) -> None:
        for name in (
            "late_native_valid_disaster_boundary_preserves_complete_house_runtime",
            "network_ticks_remain_explicit_and_zero_calls_are_inert",
            "original_animation_and_growth_controls_are_transactionally_refused",
        ):
            result = ControlRun(ROOT, output, self.controls).run(
                name,
                [str(self.controls), "--ignored", "--exact", name, "--nocapture"],
                {
                    "OTTD_HOUSE_ROLLBACK_SAVE": str(ready),
                    "OTTD_HOUSE_CONTROL_DIR": str(corpus),
                },
            )
            require_test(result.stdout, name)
