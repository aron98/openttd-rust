# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through scripts/check-order-state.py.
from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from scripts.context_ci_support import exact
from scripts.gameplay_foundations import digest, require_test
from scripts.grf_control_evidence import sequence, text
from scripts.order_state_pair import compare_case
from scripts.order_state_processes import execute
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    read_json,
    replace,
    run,
    write_json,
)


@dataclass(frozen=True, slots=True)
class OrderRun:
    root: Path
    output: Path
    oracle: Path
    manifest: Json
    fixtures: Json

    def descriptor(self, name: str) -> Json:
        return at(self.fixtures, ("descriptors", name))

    def fixture(self, source: Path, name: str) -> Path:
        directory = self.output / name
        directory.mkdir(parents=True)
        descriptor = directory / "actions.json"
        write_json(descriptor, self.descriptor(name))
        self.inputs(source, directory)
        execute(self.oracle, source, descriptor, directory / "original")
        return directory / "original/native"

    def inputs(self, source: Path, directory: Path) -> None:
        write_json(
            directory / "inputs.json",
            {
                "save": str(source),
                "save_sha256": digest(source),
                "descriptor_sha256": digest(directory / "actions.json"),
                "oracle_sha256": digest(self.oracle),
            },
        )

    def pair(self, name: str) -> None:
        compare_case(self.manifest, self.output / name, self.output / "paired" / name)

    def prepare(self, source: Path, directory: Path) -> Path:
        _ = run(
            [
                "cmake",
                f"-DORACLE={self.oracle}",
                f"-DRUN_DIR={directory}",
                f"-DINPUT={source}",
                f"-DCONFIG={self.root / 'scripts/reference.cfg'}",
                "-DPREPARE=ON",
                "-DVECTORS=OFF",
                "-P",
                str(self.root / "scripts/run-depot-runtime-reference.cmake"),
            ],
            directory.parent / (directory.name + "-command"),
        )
        return directory / "save/autosave/exit.sav"

    def cli(self) -> str:
        return text(at(self.manifest, ("cli", "executable")))


def single_player(job: OrderRun) -> None:
    prepared = job.prepare(
        job.root / "fixtures/replay/clear-v362.sav", job.output / "sp/prepared"
    )
    fleet = job.fixture(prepared, "sp/fleet") / "fleet.sav"
    for value in sequence(at(job.fixtures, ("sp_order",))):
        name = text(value)
        initial = job.fixture(fleet, "sp/setup/" + name) / "initial.sav"
        _ = job.fixture(initial, "sp/cases/" + name)
        job.pair("sp/cases/" + name)
        print(f"PASS original order lifecycle {name}", flush=True)


def orphan(job: OrderRun) -> None:
    source = job.output / "sp/fleet/original/native/fleet.sav"
    initial = job.fixture(source, "orphan/setup") / "orphan.sav"
    _ = job.fixture(initial, "orphan/orphan")
    job.pair("orphan/orphan")


def hangar(job: OrderRun) -> None:
    output = job.output / "hangar"
    output.mkdir()
    source = job.root / "fixtures/replay/clear-v362.sav"
    paused = output / "paused.sav"
    edits = output / "pause-edit.json"
    write_json(
        edits,
        {
            "schema_version": 1,
            "edits": [
                {
                    "kind": "field",
                    "chunk": "DATE",
                    "record": 0,
                    "path": ["pause_mode"],
                    "value": {"unsigned": 1},
                }
            ],
        },
    )
    _ = run(
        [
            job.cli(),
            "edit-world",
            str(source),
            str(edits),
            str(paused),
            "--compression",
            "none",
        ],
        output / "pause-command",
    )
    values: list[Json] = []
    for label, path in (("before", source), ("after", paused)):
        result = run([job.cli(), "world", str(path), "--view", "saved"], output / label)
        _ = (output / (label + ".json")).write_text(result.stdout)
        values.append(read_json(output / (label + ".json")))
    if at(values[0], ("chunks", "DATE", "records", "0", "pause_mode")) != 0:
        raise WorldCheckError("Hangar preparation source pause differs")
    replace(values[0], ("chunks", "DATE", "records", "0", "pause_mode"), 1)
    if not exact(values[0], values[1]):
        raise WorldCheckError("Hangar pause preparation altered other state")
    airport = job.fixture(paused, "hangar/airport") / "airport.sav"
    prepared = job.prepare(airport, output / "prepared")
    fleet = job.fixture(prepared, "hangar/fleet") / "fleet.sav"
    initial = job.fixture(fleet, "hangar/setup") / "initial.sav"
    result = job.fixture(initial, "hangar/case")
    final = at(read_json(result / "results.json"), ("final",))
    vehicles = sequence(at(final, ("vehicles",)))
    created = read_json(fleet.parent / "results.json")
    primary_ids = [at(created, ("actions", i, "result", "vehicle")) for i in (0, 1)]
    orders = [
        at(row, ("current_order", "type"))
        for identity in primary_ids
        for row in vehicles
        if at(row, ("id",)) == identity
    ]
    if (
        orders != [2, 5]
        or len(sequence(at(final, ("backups",)))) != 1
        or at(final, ("backups", 0, "tile")) != 1585
    ):
        raise WorldCheckError("Original hangar/depot namespace witness differs")
    job.pair("hangar/case")


def airport_geometry(job: OrderRun) -> None:
    output = job.output / "airport"
    output.mkdir()
    source = job.output / "sp/fleet/original/native/fleet.sav"
    for label in ("canonical", "vectors"):
        directory = output / label
        argv = ["env", f"OTTD_WORLD_PATH={directory / 'world.json'}"]
        if label == "vectors":
            argv.append(f"OTTD_ORDER_STATE_PATH={directory / 'order.json'}")
        argv += [
            "cmake",
            f"-DORACLE={job.oracle}",
            f"-DRUN_DIR={directory}",
            f"-DINPUT={source}",
            f"-DCONFIG={job.root / 'scripts/reference.cfg'}",
            "-DPREPARE=OFF",
            "-DVECTORS=OFF",
            "-P",
            str(job.root / "scripts/run-depot-runtime-reference.cmake"),
        ]
        _ = run(argv, output / (label + "-command"))
    observed = at(read_json(output / "vectors/order.json"), ("airport_geometry",))
    if len(sequence(at(observed, ("rows",)))) != 40:
        raise WorldCheckError("Airport geometry membership differs")
    write_json(output / "native.json", observed)
    selector = text(at(job.manifest, ("airport_test",)))
    result = run(
        [
            "env",
            f"ORDER_CASE_INPUT={source}",
            f"ORDER_AIRPORT_OUTPUT={output / 'rust.json'}",
            text(at(job.manifest, ("runner", "executable"))),
            "--exact",
            selector,
            "--ignored",
            "--nocapture",
        ],
        output / "rust-command",
    )
    require_test(result.stdout, selector)
    for label, left, right in (
        ("geometry", output / "native.json", output / "rust.json"),
        ("saved", output / "canonical/world.json", output / "vectors/world.json"),
        (
            "live",
            output / "canonical/depot-runtime.json",
            output / "vectors/depot-runtime.json",
        ),
    ):
        _ = run(
            [job.cli(), "compare", str(left), str(right)], output / (label + "-compare")
        )
