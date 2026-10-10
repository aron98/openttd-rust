from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path

from scripts.grf_control_evidence import sequence, text
from scripts.language_ci_compare import compare, mapping
from scripts.tree_ci_run import TreeRun
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json

PREP_SELECTOR = (
    "commands::terrain_run::corpus::preparation::prepare_sparse_town_fixture"
)


@dataclass
class Preparation:
    run: TreeRun
    layout: Json
    lib: Path
    cli: Path
    completed: dict[str, Path] = field(default_factory=dict)
    active: set[str] = field(default_factory=set)

    def resolve(self, reference: Json) -> Path:
        match at(reference, ("kind",)):
            case "tracked":
                name = text(at(reference, ("path",)))
                path = self.run.job.root / name
                if not path.resolve().is_relative_to(self.run.job.root.resolve()):
                    raise WorldCheckError("Tracked tree input escapes root")
                return path
            case "checkpoint":
                return self.native(text(at(reference, ("case",)))) / (
                    text(at(reference, ("label",))) + ".sav"
                )
            case "edit":
                return self.edit(text(at(reference, ("name",))))
            case "sparse_town":
                path = self.run.job.output / "preparation/sparse-town.sav"
                if not path.exists():
                    path.parent.mkdir(parents=True, exist_ok=True)
                    self.run.test(
                        self.lib,
                        PREP_SELECTOR,
                        {
                            "TREE_PREP_INPUT": str(
                                self.run.job.root / "fixtures/replay/clear-v362.sav"
                            ),
                            "TREE_PREP_OUTPUT": str(path),
                        },
                    )
                return path
            case _:
                raise WorldCheckError("Unknown tree input preparation")

    def native(self, identity: str) -> Path:
        if identity in self.completed:
            return self.completed[identity]
        if identity in self.active:
            raise WorldCheckError("Cycle in tree native preparation graph")
        entries = [
            row
            for row in sequence(at(self.layout, ("native_runs",)))
            if at(row, ("id",)) == identity
        ]
        if len(entries) != 1:
            raise WorldCheckError("Unknown or duplicate tree native recipe")
        entry = entries[0]
        self.active.add(identity)
        source = self.resolve(at(entry, ("input",)))
        protocol = (
            self.run.job.output / "protocols" / (identity.replace("/", "-") + ".json")
        )
        protocol.parent.mkdir(exist_ok=True)
        write_json(protocol, at(entry, ("protocol",)))
        observed = at(entry, ("observe",))
        if not isinstance(observed, bool):
            raise WorldCheckError("Tree observer mode is not boolean")
        result = self.run.native(identity, source, protocol, observe=observed)
        self.completed[identity] = result
        self.active.remove(identity)
        if identity.startswith("corpus/canonical-"):
            self.validate_edit(identity.removeprefix("corpus/canonical-"), result)
        return result

    def edit(self, name: str) -> Path:
        entry = at(self.layout, ("edits", name))
        directory = self.run.job.output / "preparation" / name
        output = directory / "input.sav"
        if output.exists():
            return output
        source = self.resolve(at(entry, ("input",)))
        directory.mkdir(parents=True)
        write_json(directory / "edits.json", at(entry, ("edits",)))
        _ = self.run.job.run(
            "edit/" + name,
            [
                str(self.cli),
                "edit-world",
                str(source),
                str(directory / "edits.json"),
                str(output),
                "--compression",
                "none",
            ],
        )
        result = self.run.job.run(
            "intended/" + name, [str(self.cli), "world", str(output), "--view", "saved"]
        )
        _ = (directory / "intended.world.json").write_text(result.stdout)
        return output

    def validate_edit(self, name: str, native: Path) -> None:
        expected = read_json(
            self.run.job.output / "preparation" / name / "intended.world.json"
        )
        actual = read_json(native / "final.world.json")
        delta = at(self.layout, ("edits", name, "canonical_delta"))
        match at(delta, ("kind",)):
            case "identity":
                compare(actual, expected)
            case "climate_log":
                records = mapping(at(actual, ("chunks", "GLOG", "records")))
                row = records.pop("2")
                compare(
                    row,
                    {
                        "at": 1,
                        "tick": 0,
                        "action": [
                            {
                                "ct": 0,
                                "emergency": [],
                                "grfadd": [],
                                "grfbug": [],
                                "grfcompat": [],
                                "grfmove": [],
                                "grfparam": [],
                                "grfrem": [],
                                "mode": [
                                    {
                                        "mode.landscape": at(delta, ("climate",)),
                                        "mode.mode": 1,
                                    }
                                ],
                                "oldver": [],
                                "revision": [],
                                "setting": [],
                            }
                        ],
                    },
                )
                compare(actual, expected)
            case _:
                raise WorldCheckError("Unknown canonical tree fixture delta")
