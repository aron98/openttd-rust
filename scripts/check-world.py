# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: bash scripts/check-worlds.sh

from __future__ import annotations

import argparse
import copy
import hashlib
import shutil
from dataclasses import dataclass
from pathlib import Path
from scripts.world_check_support import ROOT, Json, WorldCheckError, at, content_mutation, decode_json, read_json, replace, resaved_checkpoint, run, write_json


@dataclass(frozen=True, slots=True)
class Matrix:
    oracle: Path
    cli: Path
    artifacts: Path
    original: Path = ROOT / ".reference/build/openttd"

    def compare(self, expected: Path, actual: Path, log: Path, code: int = 0) -> None:
        result = run([str(self.cli), "compare", str(expected), str(actual)], log, code)
        if code == 1 and "$.chunks.PLYR.records.0.money" not in result.stderr:
            raise WorldCheckError(f"Comparator negative failed for the wrong reason: {log}")

    def export(self, save: Path, view: str, output: Path, log: Path) -> None:
        result = run([str(self.cli), "world", str(save), "--view", view], log)
        _ = output.write_text(result.stdout)

    def native(self, save: Path, case: Path, *, modded: bool = False, content: bool = True, expected: int = 0, corrupt: str | None = None, instrumented: bool = True) -> Path:
        native = case / "native"
        native.mkdir(parents=True)
        (native / "ai").mkdir()
        _ = shutil.copytree(ROOT / "fixtures/world/world-probe", native / "ai/WorldProbe")
        _ = shutil.copytree(ROOT / "fixtures/world/world-game", native / "game/WorldGame")
        if modded and content:
            (native / "newgrf").mkdir()
            _ = shutil.copyfile(ROOT / "fixtures/content/contract-speed.grf", native / "newgrf/contract-speed.grf")
        config = case / "reference.cfg"
        _ = config.write_text((ROOT / "scripts/reference.cfg").read_text())
        oracle = self.oracle if instrumented else self.original
        runner = "check-world-reference.cmake" if instrumented else "run-reference.cmake"
        command = ["cmake", f"-DORACLE={oracle}", f"-DRUN_DIR={native}", f"-DCONFIG={config}",
                   f"-DINPUT={save}", "-DTICKS=1", "-P", str(ROOT / "scripts" / runner)]
        if corrupt is not None:
            command = ["cmake", "-E", "env", f"OTTD_WORLD_CORRUPT_DERIVED={corrupt}", *command]
        _ = run(command, case / "native-command", expected)
        if expected == 0 and "[grf:0]" in (native / "stderr.log").read_text():
            raise WorldCheckError(f"Native content warning: {case}")
        return native

    def baseline(self, name: str, save: Path, *, modded: bool = False) -> Path:
        case = self.artifacts / name / "baseline"
        native = self.native(save, case, modded=modded)
        self.export(save, "derived", case / "rust-derived.json", case / "derived-command")
        self.compare(native / "derived.json", case / "rust-derived.json", case / "derived-compare")
        self.export(native / "save/autosave/exit.sav", "saved", case / "rust-saved.json", case / "saved-command")
        self.compare(native / "world.json", case / "rust-saved.json", case / "saved-compare")
        resaved_checkpoint(self, native, case / "resaved", modded=modded)
        if name == "populated":
            metadata = read_json(ROOT / "crates/ottd-save/src/world/native-schema-v362.json")
            if not isinstance(metadata, dict):
                raise WorldCheckError("Native semantic metadata must be an object")
            _ = metadata.pop("native_commit")
            write_json(case / "expected-schema.json", metadata)
            self.compare(case / "expected-schema.json", native / "schema.json", case / "schema-compare")
        edits = case / "no-op.json"
        write_json(edits, {"schema_version": 1, "edits": []})
        rewritten = case / "no-op.sav"
        _ = run([str(self.cli), "edit-world", str(save), str(edits), str(rewritten)], case / "no-op-command")
        no_op = self.native(rewritten, case / "no-op-reload", modded=modded)
        self.compare(native / "world.json", no_op / "world.json", case / "no-op-saved-compare")
        self.compare(native / "derived.json", no_op / "derived.json", case / "no-op-derived-compare")
        return native

    def mutations(self, base: Path, native_baseline: Path) -> None:
        case = self.artifacts / "mutations"
        case.mkdir()
        specs: list[tuple[str, str, int, tuple[str | int, ...], str, Json]] = [
            ("vehicles", "VEHS", 12, ("roadveh", 0, "common", 0, "name"), "bytes", list(b"World Mutated Vehicle")),
            ("companies", "PLYR", 0, ("name",), "bytes", list(b"World Mutated Company")),
            ("towns", "CITY", 0, ("name",), "bytes", list(b"World Mutated Town")),
            ("industries", "INDY", 0, ("prod_level",), "unsigned", 17),
            ("stations", "STNN", 0, ("normal", 0, "base", 0, "name"), "bytes", list(b"World Mutated Station")),
            ("orders", "ORDL", 0, ("orders", 0, "wait_time"), "unsigned", 37),
            ("cargo", "CAPA", 0, ("feeder_share",), "signed", 123),
            ("infrastructure", "DEPT", 0, ("name",), "bytes", list(b"World Mutated Depot")),
        ]
        edits: list[Json] = [{"kind": "field", "chunk": chunk, "record": record, "path": list(path), "value": {kind: value}}
                             for _, chunk, record, path, kind, value in specs]
        write_json(case / "edits.json", {"schema_version": 1, "edits": edits})
        edited = case / "edited.sav"
        _ = run([str(self.cli), "edit-world", str(base), str(case / "edits.json"), str(edited)], case / "edit-command")
        native = self.native(edited, case)
        self.export(edited, "derived", case / "rust-derived.json", case / "derived-command")
        self.compare(native / "derived.json", case / "rust-derived.json", case / "derived-compare")
        self.export(native / "save/autosave/exit.sav", "saved", case / "rust-saved.json", case / "saved-command")
        self.compare(native / "world.json", case / "rust-saved.json", case / "saved-compare")
        expected = read_json(native_baseline / "world.json")
        assertions: list[Json] = []
        for family, chunk, record, path, _, value in specs:
            full_path = ("chunks", chunk, "records", str(record), *path)
            previous = at(expected, full_path)
            if previous == value:
                raise WorldCheckError(f"Mutation did not change {family}")
            replace(expected, full_path, value)
            assertions.append({"family": family, "path": list(full_path), "before": previous, "after": value})
        write_json(case / "expected-native.json", expected)
        self.compare(case / "expected-native.json", native / "world.json", case / "change-compare")
        write_json(case / "assertions.json", assertions)
        resaved_checkpoint(self, native, case / "resaved")
        for assertion in assertions:
            if not isinstance(assertion, dict):
                raise WorldCheckError("Expected mutation assertion")
            write_json(case / f"{assertion['family']}.json", assertion)

    def negatives(self, base_native: Path, modded: Path) -> None:
        expected = read_json(base_native / "world.json")
        changed = copy.deepcopy(expected)
        replace(changed, ("chunks", "PLYR", "records", "0", "money"), -999)
        write_json(self.artifacts / "negative-mutation.json", changed)
        self.compare(self.artifacts / "negative-mutation.json", base_native / "world.json", self.artifacts / "negative-mutation", 1)
        missing = copy.deepcopy(expected)
        record = at(missing, ("chunks", "PLYR", "records", "0"))
        if not isinstance(record, dict):
            raise WorldCheckError("Expected company object")
        del record["money"]
        write_json(self.artifacts / "negative-missing-field.json", missing)
        self.compare(self.artifacts / "negative-missing-field.json", base_native / "world.json", self.artifacts / "negative-missing-field", 1)
        missing_content = self.native(modded, self.artifacts / "negative-missing-content", modded=True, content=False, expected=1)
        if "[grf:0]" not in (missing_content / "stderr.log").read_text():
            raise WorldCheckError("Missing-content negative did not observe a native content warning")
        stale = run(["cmake", f"-DORACLE={self.oracle}", f"-DRUN_DIR={base_native}", f"-DCONFIG={ROOT / 'scripts/reference.cfg'}",
             f"-DINPUT={ROOT / 'fixtures/world/populated-v362.sav'}", "-DTICKS=1", "-P", str(ROOT / "scripts/check-world-reference.cmake")], self.artifacts / "negative-stale", 1)
        if "stale world oracle output" not in stale.stderr:
            raise WorldCheckError("Stale-output negative failed for the wrong reason")
        marker = run(["cmake", f"-DORACLE={self.oracle}", f"-DRUN_DIR={self.artifacts / 'missing-builder-output'}",
             f"-DINPUT={ROOT / 'fixtures/world/populated-v362.sav'}", "-DRELOAD=ON", "-P", str(ROOT / "scripts/check-world-builder.cmake")], self.artifacts / "negative-builder-marker", 1)
        if "did not restore its saved script state" not in marker.stderr:
            raise WorldCheckError("Builder-marker negative failed for the wrong reason")
        for control, fixture, key in [("group-children", "populated-extended", "groups"),
                                      ("cargo-cache", "storage-payment", "cargo_lists"),
                                      ("cargo-payment", "storage-payment", "cargo_payments")]:
            case = self.artifacts / f"negative-{control}"
            save = ROOT / f"fixtures/world/{fixture}-v362.sav"
            native = self.native(save, case, corrupt=control)
            self.export(save, "derived", case / "rust-derived.json", case / "derived-command")
            result = run([str(self.cli), "compare", str(native / "derived.json"), str(case / "rust-derived.json")], case / "compare", 1)
            if f"$.{key}" not in result.stderr or f"WORLD_DERIVED_CONTROL {control}" not in (native / "stderr.log").read_text():
                raise WorldCheckError(f"Native cache control failed for the wrong reason: {control}")
            baseline_name = "extended" if fixture == "populated-extended" else "storage-payment"
            self.compare(self.artifacts / baseline_name / "baseline/native/world.json", native / "world.json", case / "saved-unchanged")

    def storage_payment_map(self, save: Path, baseline: Path) -> None:
        case = self.artifacts / "storage-payment-map"
        case.mkdir()
        snapshot = run([str(self.cli), "snapshot", str(save)], case / "map-command")
        tiles = at(decode_json(snapshot.stdout), ("map", "tiles"))
        if not isinstance(tiles, list):
            raise WorldCheckError("Expected map tiles")
        selected: tuple[int, dict[str, Json]] | None = None
        for index, tile in enumerate(tiles):
            if isinstance(tile, dict) and isinstance(tile["type"], int) and isinstance(tile["m5"], int):
                if tile["type"] >> 4 == 2 and tile["m5"] >> 6 == 0:
                    selected = (index, tile)
                    break
        if selected is None:
            raise WorldCheckError("Synthetic fixture has no normal road tile")
        index, tile = selected
        old = tile["m6"]
        if not isinstance(old, int):
            raise WorldCheckError("Expected road m6 byte")
        tile["m6"] = (old & ~56) | (8 if old & 56 == 16 else 16)
        write_json(case / "edits.json", {"schema_version": 1, "edits": [
            {"kind": "field", "chunk": "PSAC", "record": 0, "path": ["storage", 0], "value": {"unsigned": 2468}},
            {"kind": "field", "chunk": "CAPY", "record": 0, "path": ["route_profit"], "value": {"signed": 54321}},
            {"kind": "field", "chunk": "CAPA", "record": 3, "path": ["feeder_share"], "value": {"signed": 9223372036854775807}},
            {"kind": "field", "chunk": "CAPA", "record": 2, "path": ["feeder_share"], "value": {"signed": 1}},
            {"kind": "tile", "index": index, "value": tile}]})
        edited = case / "edited.sav"
        _ = run([str(self.cli), "edit-world", str(save), str(case / "edits.json"), str(edited)], case / "edit-command")
        native = self.native(edited, case)
        self.export(edited, "derived", case / "rust-derived.json", case / "derived-command")
        self.compare(native / "derived.json", case / "rust-derived.json", case / "derived-compare")
        self.export(native / "save/autosave/exit.sav", "saved", case / "rust-saved.json", case / "saved-command")
        self.compare(native / "world.json", case / "rust-saved.json", case / "saved-compare")
        expected = read_json(baseline / "world.json")
        replace(expected, ("chunks", "PSAC", "records", "0", "storage", 0), 2468)
        replace(expected, ("chunks", "CAPY", "records", "0", "route_profit"), 54321)
        replace(expected, ("chunks", "CAPA", "records", "3", "feeder_share"), 9223372036854775807)
        replace(expected, ("chunks", "CAPA", "records", "2", "feeder_share"), 1)
        replace(expected, ("chunks", "MAPE", "bytes", index), tile["m6"])
        write_json(case / "expected-native.json", expected)
        write_json(case / "roadside.json", {"tile": index, "m6_before": old, "m6_after": tile["m6"], "source": "road_map.h:SetRoadside bits3..5"})
        self.compare(case / "expected-native.json", native / "world.json", case / "change-compare")
        aggregates = at(read_json(native / "derived.json"), ("cargo_lists",))
        if not isinstance(aggregates, list):
            raise WorldCheckError("Expected native cargo lists")
        vehicle = [entry for entry in aggregates if isinstance(entry, dict) and entry["owner"] == {"pool": "VEHS", "id": 21}]
        if len(vehicle) != 1 or vehicle[0]["feeder_share"] != 9223372036854775807:
            raise WorldCheckError("Native vehicle feeder cache did not saturate")
        write_json(case / "saturation.json", {"expected": 9223372036854775807, "native": vehicle[0]["feeder_share"]})
        resaved_checkpoint(self, native, case / "resaved")


class Arguments(argparse.Namespace):
    oracle: Path = Path()
    ottd: Path = Path()
    artifacts: Path = Path()
    original: Path = ROOT / ".reference/build/openttd"


def main() -> None:
    parser = argparse.ArgumentParser()
    _ = parser.add_argument("--oracle", type=Path, required=True)
    _ = parser.add_argument("--ottd", type=Path, required=True)
    _ = parser.add_argument("--artifacts", type=Path, required=True)
    _ = parser.add_argument("--original", type=Path, default=ROOT / ".reference/build/openttd")
    args = Arguments()
    _ = parser.parse_args(namespace=args)
    matrix = Matrix(args.oracle.resolve(), args.ottd.resolve(), args.artifacts.resolve(), args.original.resolve())
    matrix.artifacts.mkdir(parents=True, exist_ok=False)
    write_json(matrix.artifacts / "binaries.json", {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in (matrix.oracle, matrix.cli, matrix.original)})
    populated = ROOT / "fixtures/world/populated-v362.sav"
    modded = ROOT / "fixtures/world/modded-v362.sav"
    baseline = matrix.baseline("populated", populated)
    _ = matrix.baseline("extended", ROOT / "fixtures/world/populated-extended-v362.sav")
    modded_baseline = matrix.baseline("modded", modded, modded=True)
    storage = ROOT / "fixtures/world/storage-payment-v362.sav"
    storage_baseline = matrix.baseline("storage-payment", storage)
    game = ROOT / "fixtures/world/game-v362.sav"
    game_baseline = matrix.baseline("game", game)
    _ = run(["cmake", f"-DORACLE={matrix.oracle}", f"-DRUN_DIR={matrix.artifacts / 'game-script-reload'}",
             f"-DINPUT={game}", "-DRELOAD=ON", "-P", str(ROOT / "scripts/check-world-game.cmake")], matrix.artifacts / "game-script-command")
    resaved_checkpoint(matrix, matrix.artifacts / "game-script-reload", matrix.artifacts / "game-script-resaved")
    modded_edited = content_mutation(matrix, "modded", modded, modded_baseline, modded=True)
    game_edited = content_mutation(matrix, "game", game, game_baseline)
    matrix.mutations(populated, baseline)
    matrix.storage_payment_map(storage, storage_baseline)
    matrix.negatives(baseline, modded)
    for name, save, has_grf in [("eight-families", matrix.artifacts / "mutations/edited.sav", False),
                               ("storage-payment-map", matrix.artifacts / "storage-payment-map/edited.sav", False),
                               ("modded", modded_edited, True),
                               ("game", game_edited, False)]:
        _ = matrix.native(save, matrix.artifacts / "unmodified" / name, modded=has_grf, instrumented=False)
    write_json(matrix.artifacts / "summary.json", {"passed": True, "fixtures": ["populated", "extended", "modded", "storage-payment", "game"],
        "mutation_families": ["vehicles", "companies", "towns", "industries", "stations", "orders", "cargo", "infrastructure"],
        "additional_mutations": ["persistent-storage", "cargo-payment", "roadside-map-tile", "saturated-cargo-feeder"],
        "active_content_mutations": ["modded", "game"], "native_resave_derived_pairs": 14,
        "unmodified_original_loads": ["eight-families", "storage-payment-map", "modded", "game"],
        "negative_controls": ["mutation", "missing-field", "missing-content", "stale", "builder-marker", "group-children", "cargo-cache", "cargo-payment"]})
    print(f"PASS world loading/saving matrix: {matrix.artifacts}")


if __name__ == "__main__":
    main()
