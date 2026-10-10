from __future__ import annotations

import copy
import shutil
from pathlib import Path

from scripts.depot_build_archive import bounded_paths, package_raw, verify_archive
from scripts.empty_road_evidence import compare_trace
from scripts.empty_road_provenance import verify_binaries, verify_sources
from scripts.empty_road_run import EmptyRun
from scripts.gameplay_foundations import digest, require_test
from scripts.language_ci_compare import mapping
from scripts.owned_restore_freshness import relocated_stamp
from scripts.purchase_creation import integer
from scripts.shared_restore_admission import rejected
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    read_json,
    replace,
    write_json,
)


def roadside_control(before: int) -> int:
    """Change Grass/Barren using road_map.h GetRoadside's bits 3..5."""
    return before ^ 8


def controls(run: EmptyRun, hashes: Json) -> None:
    output = run.job.output / "controls"
    output.mkdir()
    expected = run.job.output / "native/ticks/after-road.world.json"
    actual = run.job.output / "rust/ticks/after-road.world.json"
    expected_sha = digest(expected)
    _ = run.job.run(
        "corruption-baseline", [run.cli, "compare", str(expected), str(actual)]
    )
    mutations: dict[str, Json] = {}
    for name, path in (
        ("roadside", ("chunks", "MAPE", "bytes", 649)),
        ("cursor", ("chunks", "DATE", "records", "0", "cur_tileloop_tile")),
        ("rng", ("chunks", "DATE", "records", "0", "random_state[0]")),
    ):
        value = read_json(expected)
        before = integer(at(value, path))
        after = roadside_control(before) if name == "roadside" else before ^ 1
        replace(value, path, after)
        changed = output / (name + ".json")
        write_json(changed, value)
        mutations[name] = {
            "field": list(path),
            "before": before,
            "after": after,
            "original_sha256": expected_sha,
            "mutant_sha256": digest(changed),
        }
        _ = run.job.run(
            "corrupt-" + name,
            [run.cli, "compare", str(changed), str(actual)],
            expected=1,
        )
    if digest(expected) != expected_sha:
        raise WorldCheckError("Empty-road corruption control changed original")
    _ = run.job.run(
        "corruption-recheck", [run.cli, "compare", str(expected), str(actual)]
    )
    write_json(output / "saved-mutations.json", mutations)
    runner = str(at(run.binaries, ("runner", "retained")))
    zero = run.job.run(
        "zero-test", [runner, "--exact", "empty_road_absent_selector", "--nocapture"]
    )
    results: dict[str, Json] = {
        "zero": rejected(
            lambda: require_test(zero.stdout, "empty_road_absent_selector"),
            "Exact ignored test",
        )
    }
    results.update(mapping(trace_controls(run, output)))
    _ = run.job.run(
        "subset",
        ["python3", str(run.job.root / "scripts/check-empty-road.py"), "--case=ticks"],
        expected=1,
    )
    stale = output / "source"
    _ = shutil.copytree(run.job.output / "source", stale)
    embedded = stale / "fixtures/road-tile/built.sav"
    data = embedded.read_bytes()
    _ = embedded.write_bytes(data[:-1] + bytes([data[-1] ^ 1]))
    results["source"] = rejected(
        lambda: verify_sources(stale, hashes), "Pinned witness source changed"
    )
    changed = copy.deepcopy(run.binaries)
    replace(changed, ("runner",), at(changed, ("cli",)))
    results["executable"] = rejected(
        lambda: verify_binaries(run.job.output, changed), "observables differ"
    )
    guard = output / "native"
    guard.mkdir()
    native = guard / "openttd"
    _ = shutil.copy2(run.job.oracle, native)
    native.chmod(0o755)
    stamp = relocated_stamp(
        (run.job.oracle.parent / "replay-build.sha256").read_text(),
        run.job.oracle,
        native,
        digest(run.job.oracle),
    )
    _ = (guard / "replay-build.sha256").write_text(stamp)
    argv = [
        "cmake",
        f"-DORACLE={native}",
        "-P",
        str(run.job.root / "scripts/check-replay-build.cmake"),
    ]
    _ = run.job.run("native-copy-baseline", argv)
    data = native.read_bytes()
    _ = native.write_bytes(data[:-1] + bytes([data[-1] ^ 1]))
    _ = run.job.run("native-copy-rejected", argv, expected=1)
    archive_controls(run, output)
    write_json(output / "admission.json", results)


def archive_controls(run: EmptyRun, output: Path) -> None:
    baseline = output / "archive-baseline"
    baseline.mkdir()
    _ = shutil.copy2(
        run.job.output / "native/ticks/results.json", baseline / "results.json"
    )
    package_raw(baseline)
    verify_archive(baseline)
    results: dict[str, Json] = {}
    for name in ("missing", "extra"):
        directory = output / ("membership-" + name)
        directory.mkdir()
        if name == "extra":
            _ = shutil.copy2(baseline / "results.json", directory / "results.json")
            _ = shutil.copy2(baseline / "results.json", directory / "extra.json")

        def check_members(directory: Path = directory) -> None:
            _ = bounded_paths(directory, {"results.json"})

        results[name] = rejected(check_members, "membership differs")
    for name in ("raw", "archive", "index"):
        changed = output / ("archive-" + name)
        _ = shutil.copytree(baseline, changed)
        target = (
            changed
            / {
                "raw": "results.json",
                "archive": "evidence.tar.gz",
                "index": "evidence-index.json",
            }[name]
        )
        if name == "index":
            value = read_json(target)
            replace(value, ("count",), 0)
            write_json(target, value)
        else:
            data = target.read_bytes()
            _ = target.write_bytes(data[:-1] + bytes([data[-1] ^ 1]))
        results[name] = rejected(
            lambda changed=changed: verify_archive(changed), "archive"
        )
    write_json(output / "archive-controls.json", results)


def trace_controls(run: EmptyRun, output: Path) -> Json:
    results: dict[str, Json] = {}
    native_trace = read_json(run.job.output / "native/ticks/results.json")
    rust_trace = read_json(run.job.output / "rust/ticks/results.json")
    for name in ("missing-action", "extra-checkpoint", "host-metadata"):
        altered = copy.deepcopy(native_trace)
        mutate_trace(altered, name)
        write_json(output / (name + ".json"), altered)
        results[name] = rejected(
            lambda altered=altered: compare_trace(
                run.protocol("ticks"), altered, rust_trace
            ),
            "Empty-road",
        )
    return results


def mutate_trace(altered: Json, name: str) -> None:
    match name:
        case "missing-action":
            actions = at(altered, ("actions",))
            if not isinstance(actions, list):
                raise WorldCheckError("Missing mutation action array")
            _ = actions.pop()
        case "extra-checkpoint":
            checkpoints = at(altered, ("checkpoints",))
            if not isinstance(checkpoints, list):
                raise WorldCheckError("Missing mutation checkpoint array")
            checkpoints.append(copy.deepcopy(checkpoints[-1]))
        case "host-metadata":
            before = at(altered, ("actions", 0, "before"))
            if not isinstance(before, dict):
                raise WorldCheckError("Missing mutation runtime")
            before["native_metadata"] = 1
        case _:
            raise WorldCheckError("Unknown empty-road trace mutation")
