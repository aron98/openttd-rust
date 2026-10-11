# SPDX-License-Identifier: GPL-2.0-only
"""Private tree-family full-loop proof; native must be built with tree_growth.patch."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()

def write(path: Path, value) -> None:
    path.write_text(json.dumps(value, indent=2) + "\n")

def run(command: list[str], directory: Path, *, env: dict[str, str] | None = None, expected: int = 0) -> None:
    directory.mkdir(parents=True, exist_ok=False)
    write(directory / "invocation.json", {"argv": command, "cwd": str(ROOT), "environment": env or {}})
    with (directory / "stdout.log").open("w") as stdout, (directory / "stderr.log").open("w") as stderr:
        result = subprocess.run(command, cwd=ROOT, env=os.environ | (env or {}), stdout=stdout, stderr=stderr, timeout=900, check=False)
    write(directory / "exit.json", {"actual": result.returncode, "expected": expected})
    if result.returncode != expected:
        raise RuntimeError(f"command failed ({result.returncode}): {directory}")

def native(binary: Path, save: Path, protocol: dict, directory: Path, *, observe: bool = True) -> Path:
    directory.mkdir(parents=True)
    path = directory / "actions.json"
    write(path, protocol)
    env = {"OTTD_REPLAY_PATH": str(path), "OTTD_REPLAY_OUTPUT": str(directory)}
    if observe:
        env["OTTD_TREE_GROWTH_EVENTS"] = str(directory / "events.jsonl")
    write(directory / "bindings.json", {"binary": str(binary), "binary_sha256": digest(binary), "input": str(save), "input_sha256": digest(save), "protocol_sha256": digest(path)})
    run([str(binary), "-X", "-x", "-c", str(ROOT / "scripts/reference.cfg"), "-vnull:ticks=8", "-snull", "-mnull", "-g", str(save), "-d", "sl=2"], directory / "process", env=env)
    if not (directory / "final.sav").is_file():
        raise RuntimeError(f"native output missing: {directory}")
    return directory / "final.sav"

def compare(left: Path, right: Path, receipt: Path) -> None:
    if json.loads(left.read_bytes()) != json.loads(right.read_bytes()):
        raise RuntimeError(f"mismatch: {left} {right}")
    write(receipt, {"equal": True, "expected": str(left), "actual": str(right), "expected_sha256": digest(left), "actual_sha256": digest(right)})

def command(kind: str, **values) -> dict:
    return {"op": "command", "request": {"company": 0, "mode": "post", "command": {"kind": kind, **values}}}

def plan(actions: list[dict]) -> dict:
    return {"schema_version": 1, "actions": [{"ordinal": index, **action} for index, action in enumerate(actions)]}

def check_case(native_bin: Path, cli: Path, test: Path, seed: Path, directory: Path, ticks: int) -> Path:
    protocol = plan([{"op": "tick", "count": ticks // 2}, {"op": "checkpoint", "label": "mid"}, {"op": "tick", "count": ticks - ticks // 2}])
    native(native_bin, seed, protocol, directory / "native")
    run([str(cli), "replay-world", str(seed), str(directory / "native/actions.json"), str(directory / "rust")], directory / "rust-command")
    for label in ["initial", "mid", "final"]:
        for view in ["world", "derived"]:
            compare(directory / f"native/{label}.{view}.json", directory / f"rust/{label}.{view}.json", directory / f"equal-{label}-{view}.json")
    run([str(test), "--ignored", "--exact", "native_tree_events_match_complete_world_ticks", "--nocapture"], directory / "event-test", env={"OTTD_TREE_NATIVE_DIR": str(directory / "native")})
    proof = json.loads((directory / "native/rust-event-proof.json").read_bytes())
    if proof["calls"] != ticks or proof["callbacks"] <= 0 or not proof["whole_world_equal"] or not proof["derived_equal"]:
        raise RuntimeError("native event test produced incomplete proof")
    remaining = plan([{"op": "tick", "count": ticks - ticks // 2}])
    native(native_bin, directory / "rust/mid.sav", remaining, directory / "rust-to-native")
    run([str(cli), "replay-world", str(directory / "native/mid.sav"), str(directory / "rust-to-native/actions.json"), str(directory / "native-to-rust")], directory / "native-to-rust-command")
    for lane in ["rust-to-native", "native-to-rust"]:
        for view in ["world", "derived"]:
            compare(directory / f"native/final.{view}.json", directory / f"{lane}/final.{view}.json", directory / f"equal-{lane}-{view}.json")
    native(native_bin, seed, protocol, directory / "observer-off", observe=False)
    for label in ["initial", "mid", "final"]:
        for view in ["world", "derived"]:
            compare(directory / f"native/{label}.{view}.json", directory / f"observer-off/{label}.{view}.json", directory / f"equal-observer-off-{label}-{view}.json")
    return directory / "native/final.sav"

def lifecycle(directory: Path) -> None:
    entries = {}
    transitions = {}
    branches = set()
    grass = 0
    for line in (directory / "native/events.jsonl").read_text().splitlines():
        event = json.loads(line)
        if event["phase"] != "tile_tree":
            continue
        tile = event["tile"]
        if event["edge"] == "enter":
            entries[tile] = event
            continue
        before = entries.pop(tile)
        pair = [before["raw"][6] & 7, event["raw"][6] & 7]
        if pair[0] != pair[1]:
            transitions.setdefault(str(tile), []).append(pair)
        if before["random"] != event["random"]:
            branches.add(event["random"][1] & 7)
        if before["raw"][3] != event["raw"][3]:
            grass += 1
    complete = [tile for tile, changes in transitions.items() if 27 <= int(tile) // 64 <= 32 and 27 <= int(tile) % 64 <= 32 and changes[:7] == [[0, 1], [1, 2], [2, 3], [3, 4], [4, 5], [5, 6], [6, 0]]]
    if not complete or branches != set(range(8)) or not grass:
        raise RuntimeError("native corpus missed a planted lifecycle, RNG branch, or grass update")
    write(directory / "lifecycle.json", {"complete_original_planted_lifecycles": complete, "all_grown_random_branches": sorted(branches), "grass_changes": grass, "transitions": transitions})

def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--native", type=Path, required=True)
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--test", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    binary, cli, test, output = args.native.resolve(), args.cli.resolve(), args.test.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    write(output / "binaries.json", {str(p): digest(p) for p in [binary, cli, test]})
    fixture = {"profile": "clear", "paused": False, "terrain_growth": True, "date": {"year": 2100, "month": 0, "day": 1, "fract": 0}, "tick": 0,
        "trees": {"tiles": [{"tile": (10 + stage) * 64 + 10 + count, "type": stage, "count": count, "ground": (count - 1) % 2, "stage": stage, "density": (count - 1) % 4} for stage in range(7) for count in range(1, 5)], "towns": [], "threshold": 20, "magic_bulldozer": False}}
    seed = native(binary, ROOT / "fixtures/replay/clear-v362.sav", {"schema_version": 1, "fixture": fixture, "actions": []}, output / "prepare-fixture", observe=False)
    prepared = native(binary, seed, plan([command("tree_growth_plant", tile=32*64+32, start_tile=27*64+27, type=0)]), output / "prepare-command", observe=False)
    receipt = json.loads((output / "prepare-command/results.json").read_bytes())["actions"][0]["receipt"]
    if receipt["posted"] is not True or receipt["result"]["success"] is not True:
        raise RuntimeError(f"original plant command failed: {receipt}")
    check_case(binary, cli, test, prepared, output / "no-spread", 32768)
    lifecycle(output / "no-spread")
    corrupt = output / "corrupt-cursor"
    corrupt.mkdir()
    shutil.copy2(output / "no-spread/native/initial.sav", corrupt / "initial.sav")
    with (output / "no-spread/native/events.jsonl").open() as stream:
        first = json.loads(stream.readline())
    if first["phase"] != "vehicle_ticks" or first["edge"] != "enter":
        raise RuntimeError("cursor corruption witness expects first vehicle entry")
    first["cursor"] = 0
    (corrupt / "events.jsonl").write_text(json.dumps(first) + "\n")
    run([str(test), "--ignored", "--exact", "native_tree_events_match_complete_world_ticks", "--nocapture"], corrupt / "command", env={"OTTD_TREE_NATIVE_DIR": str(corrupt)}, expected=101)
    if (corrupt / "rust-event-proof.json").exists():
        raise RuntimeError("corrupt event produced successful proof")
    write(corrupt / "rejected.json", {"actual_cursor_mutation_rejected": True})
    frozen = native(binary, prepared, plan([command("tree_growth_setting", value=3)]), output / "prepare-no-growth", observe=False)
    check_case(binary, cli, test, frozen, output / "no-growth", 4096)
    paused = native(binary, prepared, plan([command("pause", mode=0, paused=True)]), output / "prepare-paused", observe=False)
    for name, source, count in [("zero", prepared, 0), ("paused", paused, 8)]:
        directory = output / name
        native(binary, source, plan([{"op": "tick", "count": count}]), directory / "native")
        run([str(cli), "replay-world", str(source), str(directory / "native/actions.json"), str(directory / "rust")], directory / "rust-command")
        for view in ["world", "derived"]:
            compare(directory / f"native/final.{view}.json", directory / f"rust/final.{view}.json", directory / f"equal-{view}.json")
        if (directory / "native/events.jsonl").exists():
            raise RuntimeError(f"paused/zero call emitted gameplay callback: {name}")
    failure = output / "late-failure"
    failure.mkdir()
    edit = failure / "edit.json"
    write(edit, {"schema_version": 1, "edits": [{"kind": "field", "chunk": "DATE", "record": 0, "path": ["competitors_interval"], "value": {"unsigned": 2}}]})
    source = failure / "input.sav"
    run([str(cli), "edit-world", str(prepared), str(edit), str(source)], failure / "prepare")
    protocol = failure / "actions.json"
    write(protocol, plan([{"op": "tick", "count": 2}]))
    before = digest(source)
    run([str(cli), "replay-world", str(source), str(protocol), str(failure / "rejected-output")], failure / "command", expected=1)
    if source.exists() and digest(source) == before and not (failure / "rejected-output").exists() and "tick_timer" in (failure / "command/stderr.log").read_text():
        write(failure / "rollback.json", {"late_tick_failure": True, "input_unchanged": True, "output_unpublished": True})
    else:
        raise RuntimeError("late-failure publication/rollback control failed")
    write(output / "complete.json", {"passed": True, "scope": "temperate settings 0/3 grass/rough tree callbacks through WorldTick", "native_state_loop_calls": 36864})

if __name__ == "__main__":
    main()
