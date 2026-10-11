"""Original-command preparation and passive complete-world house observations."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from scripts.gameplay_foundations import digest
from scripts.grf_control_run import ControlRun
from scripts.tree_growth_run import plan
from scripts.world_check_support import Json, WorldCheckError, write_json

ROOT = Path(__file__).resolve().parents[1]
CONFIG = """[gui]
autosave = off
autosave_on_exit = false
threaded_saves = false
[misc]
language = english.lng
savegame_format = none
survey_participation = no
[game_creation]
map_x = 6
map_y = 6
generation_seed = 12345
starting_year = 2100
se_flat_world_height = 1
landscape = temperate
tree_placer = 0
[construction]
freeform_edges = true
extra_tree_placement = 3
[difficulty]
max_no_competitors = 0
industry_density = 0
disasters = false
economy = false
subsidy_duration = 0
[economy]
type = 0
inflation = false
infrastructure_maintenance = false
town_growth_rate = 0
town_cargogen_mode = 0
town_cargo_scale = 100
place_houses = 1
[network]
server_game_type = local
"""


def command(kind: str, *, company: int = 18, **fields: Json) -> Json:
    return {
        "op": "command",
        "request": {
            "mode": "post",
            "company": company,
            "command": {"kind": kind, **fields},
        },
    }


@dataclass(frozen=True, slots=True)
class HouseNative:
    binary: Path

    def run(
        self,
        output: Path,
        actions: list[Json],
        *,
        source: Path | None = None,
        editor: bool = False,
        observe: bool = True,
    ) -> Path:
        if source is None and not editor:
            raise WorldCheckError("house preparation requires editor or original save")
        output.mkdir(parents=True)
        protocol = output / "protocol.json"
        config = output / "openttd.cfg"
        write_json(protocol, plan(actions))
        _ = config.write_text(CONFIG)
        variables = {
            "OTTD_REPLAY_PATH": str(protocol),
            "OTTD_REPLAY_OUTPUT": str(output),
            "OTTD_HOUSE_PREPARE": "1",
            "OTTD_HOUSE_CACHE": "1",
        }
        if observe:
            variables["OTTD_HOUSE_EVENTS"] = str(output / "events.jsonl")
            variables["OTTD_HOUSE_WORLD_EVENTS"] = str(output / "world-events.jsonl")
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
        ]
        if editor:
            argv.append("-e")
        if source is not None:
            argv.extend(["-g", str(source)])
        write_json(
            output / "bindings.json",
            {
                "binary": str(self.binary),
                "binary_sha256": digest(self.binary),
                "input": str(source) if source is not None else None,
                "input_sha256": digest(source) if source is not None else None,
                "protocol_sha256": digest(protocol),
                "config_sha256": digest(config),
                "editor": editor,
                "observer_enabled": observe,
            },
        )
        _ = ControlRun(ROOT, output, self.binary).run("native", argv, variables)
        result = output / "final.sav"
        if not result.is_file():
            raise WorldCheckError("original house preparation produced no final save")
        return result
