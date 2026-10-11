# SPDX-License-Identifier: GPL-2.0-only
"""Original industry commands and passive industry snapshots in StateGameLoop."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from scripts.gameplay_foundations import digest
from scripts.grf_control_run import ControlRun
from scripts.house_native import CONFIG
from scripts.tree_growth_run import plan
from scripts.world_check_support import Json, WorldCheckError, write_json

ROOT = Path(__file__).resolve().parents[1]


@dataclass(frozen=True, slots=True)
class IndustryNative:
    binary: Path
    source: Path

    def run(self, output: Path, actions: list[Json], *, observe: bool = True) -> Path:
        output.mkdir(parents=True)
        protocol = output / "protocol.json"
        config = output / "openttd.cfg"
        write_json(protocol, plan(actions))
        _ = config.write_text(CONFIG)
        variables = {
            "OTTD_REPLAY_PATH": str(protocol),
            "OTTD_REPLAY_OUTPUT": str(output),
            "OTTD_INDUSTRY_PREPARE": "1",
            "OTTD_INDUSTRY_CACHE": "1",
            "OTTD_HOUSE_CACHE": "1",
        }
        if observe:
            variables["OTTD_INDUSTRY_EVENTS"] = str(output / "industry-events.jsonl")
        argv = [
            str(self.binary),
            "-X",
            "-x",
            "-c",
            str(config),
            "-vnull:ticks=8",
            "-snull",
            "-mnull",
            "-d",
            "sl=2",
            "-g",
            str(self.source),
        ]
        write_json(
            output / "bindings.json",
            {
                "binary": str(self.binary),
                "binary_sha256": digest(self.binary),
                "input": str(self.source),
                "input_sha256": digest(self.source),
                "protocol_sha256": digest(protocol),
                "config_sha256": digest(config),
                "event_observer_enabled": observe,
            },
        )
        _ = ControlRun(ROOT, output, self.binary).run("native", argv, variables)
        result = output / "final.sav"
        if not result.is_file():
            raise WorldCheckError("Original industry run produced no final save")
        return result
