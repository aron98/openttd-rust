# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through scripts/check-order-state.py.
from __future__ import annotations

from copy import deepcopy
from pathlib import Path

from scripts.context_ci_support import exact
from scripts.depot_build_archive import bounded_paths, verify_archive
from scripts.gameplay_foundations import digest, require_test
from scripts.grf_control_evidence import sequence, text
from scripts.order_state_capture import capture_pair, compare_capture
from scripts.order_state_controls import CAPTURE_MUTATIONS
from scripts.order_state_evidence import validate_pair
from scripts.order_state_fixtures import OrderRun
from scripts.order_state_provenance import validate_native_guard
from scripts.world_check_support import Json, WorldCheckError, at, read_json, replace


def pairs(job: OrderRun) -> tuple[tuple[str, str], ...]:
    result = [
        ("sp/cases/" + text(value), "sp/cases/" + text(value))
        for value in sequence(at(job.fixtures, ("sp_order",)))
    ]
    result += [
        (name, name)
        for name in ("orphan/orphan", "hangar/case", "role/sp", "role/server")
    ]
    result += [
        ("network/" + name + "/client", "network/" + name + "/client")
        for name in ("single", "multi")
    ]
    return tuple(result)


def native_processes(job: OrderRun) -> None:
    before_paths = sorted(job.output.rglob("bindings-before.json"))
    if len(before_paths) != 47:
        raise WorldCheckError(f"Native process membership differs: {len(before_paths)}")
    for path in before_paths:
        before = read_json(path)
        if not exact(before, read_json(path.with_name("bindings-after.json"))):
            raise WorldCheckError("Native before/after binding differs")
        match before:
            case dict() as entries if entries:
                for name, sha in entries.items():
                    if digest(Path(name)) != sha:
                        raise WorldCheckError("Native process bound input changed")
                if entries.get(str(job.oracle)) != digest(job.oracle):
                    raise WorldCheckError("Native process executable binding missing")
            case _:
                raise WorldCheckError("Native process bindings missing")
        argv = sequence(read_json(path.with_name("argv.json")))
        if argv[0] != str(job.oracle):
            raise WorldCheckError("Native argv executable differs")


def boundaries(job: OrderRun) -> None:
    for name in ("boundary/sp", "boundary/server", "boundary/client/client"):
        case = job.output / name
        receipt = read_json(case / "boundary.json")
        if (
            at(receipt, ("parity_success",)) is not False
            or at(receipt, ("slot",)) != 1
            or at(receipt, ("index",)) != 0
        ):
            raise WorldCheckError("Native boundary was mislabeled parity")
        if (
            at(read_json(case / "original/process.json"), ("returncode",)) != 2
            or at(read_json(case / "command/process.json"), ("returncode",)) != 101
        ):
            raise WorldCheckError("Native/Rust crash boundary process differs")
        if (
            "NativeBackupIndex { slot: 1, index: 0 }"
            not in (case / "command/stderr.log").read_text()
            or "1 failed; 0 ignored" not in (case / "command/stdout.log").read_text()
        ):
            raise WorldCheckError("Precise native invariant boundary missing")
        if (case / "rust/results.json").exists() or digest(
            Path(text(at(receipt, ("source",))))
        ) != at(receipt, ("source_sha256",)):
            raise WorldCheckError("Native boundary published or changed its input")
        if "pn == Tpool->Get(Pool::GetRawIndex(pn->index))" not in text(
            at(read_json(case / "original/crash.json.log"), ("crash", "reason"))
        ):
            raise WorldCheckError("Original assertion witness missing")


def controls(job: OrderRun) -> None:
    directory = job.output / "controls"
    log = (directory / "zero/stdout.log").read_text()
    if (
        "0 passed; 0 failed; 0 ignored" not in log
        or at(read_json(directory / "zero/process.json"), ("returncode",)) != 0
    ):
        raise WorldCheckError("Actual zero-test control missing")
    require_test(
        (directory / "subset-command/stdout.log").read_text(),
        text(at(job.manifest, ("case_test",))),
    )
    if (
        len(sequence(at(read_json(directory / "subset/results.json"), ("actions",))))
        != 1
        or at(read_json(directory / "subset-rejected/process.json"), ("returncode",))
        != 1
    ):
        raise WorldCheckError("Actual subset rejection missing")
    expected = {
        "dummy",
        "wait",
        "travel",
        "flags",
        "duration",
        "members",
        "user",
        "id",
        "clone",
        "allocator",
        "actions",
        "object-index",
        "physical-slot",
    }
    if {path.name for path in (directory / "semantic").iterdir()} != expected:
        raise WorldCheckError("Semantic control membership differs")
    for name in expected:
        case = directory / "semantic" / name
        if (
            exact(read_json(case / "original.json"), read_json(case / "changed.json"))
            or at(read_json(case / "command/process.json"), ("returncode",)) != 1
        ):
            raise WorldCheckError("Semantic mutation was not actually rejected")
    reused = job.output / "capture/reused/client/original"
    if (
        at(read_json(reused / "process.json"), ("returncode",)) != 1
        or (reused / "received.sav").read_bytes() != b"sentinel\n"
        or (reused / "loaded.json").exists()
    ):
        raise WorldCheckError("Reused capture refusal differs")
    unarmed = job.output / "capture/unarmed/client/original"
    if (unarmed / "received.sav").exists():
        raise WorldCheckError("Unarmed capture wrote bytes")
    capture_admission(job)


def capture_admission(job: OrderRun) -> None:
    armed = read_json(job.output / "network/single/client/original/native/results.json")
    unarmed_state = read_json(
        job.output / "capture/unarmed/client/original/native/results.json"
    )
    for name, path, value in CAPTURE_MUTATIONS:
        changed = deepcopy(unarmed_state)
        replace(changed, path, value)
        directory = job.output / "controls/capture" / name
        if not exact(changed, read_json(directory / "changed.json")):
            raise WorldCheckError("Capture admission mutation changed unrelated fields")
        try:
            _ = compare_capture(armed, changed, "native-order-client")
        except WorldCheckError as error:
            if not exact(
                read_json(directory / "rejected.json"),
                {"rejected": True, "path": list(path), "diagnostic": str(error)},
            ):
                raise WorldCheckError(
                    "Capture admission rejection diagnostic differs"
                ) from error
        else:
            raise WorldCheckError("Capture admission mutation was accepted")
    if not exact(
        capture_pair(job.output),
        read_json(job.output / "capture/client-arrival-boundary.json"),
    ):
        raise WorldCheckError("Capture client arrival receipt differs")


def validate(job: OrderRun, layout: Json) -> Json:
    actions = saves = 0
    expected = pairs(job)
    comparisons = {
        str(path.parent.relative_to(job.output / "paired"))
        for path in (job.output / "paired").rglob("comparison.json")
    }
    if comparisons != {name for name, _ in expected} or len(expected) != 19:
        raise WorldCheckError("Order success case membership differs")
    for name, descriptor in expected:
        value = job.descriptor(descriptor)
        validate_pair(
            job.output / name, job.output / "paired" / name, value, job.manifest
        )
        rows = sequence(at(value, ("actions",)))
        actions += len(rows)
        saves += sum(at(row, ("op",)) == "save" for row in rows)
    if (actions, saves) != (89, 39):
        raise WorldCheckError("Order action/checkpoint totals differ")
    airport = job.output / "airport"
    if len(
        sequence(at(read_json(airport / "native.json"), ("rows",)))
    ) != 40 or not exact(
        read_json(airport / "native.json"), read_json(airport / "rust.json")
    ):
        raise WorldCheckError("Airport row membership or state differs")
    for filename in ("world.json", "depot-runtime.json"):
        if not exact(
            read_json(airport / "canonical" / filename),
            read_json(airport / "vectors" / filename),
        ):
            raise WorldCheckError("Airport observer changed state")
    rows = sequence(at(read_json(airport / "native.json"), ("rows",)))
    tuples: list[Json] = [[at(row, ("type",)), at(row, ("rotation",))] for row in rows]
    wanted: list[Json] = [
        [kind, rotation] for kind in range(10) for rotation in (0, 2, 4, 6)
    ]
    if not exact(tuples, wanted):
        raise WorldCheckError("Airport layout/rotation identities differ")
    native_processes(job)
    boundaries(job)
    controls(job)
    validate_native_guard(job.root, job.output, job.oracle)
    paths = {text(name) for name in sequence(at(layout, ("paths",)))}
    finalized = (job.output / "evidence-index.json").exists()
    if finalized:
        index = read_json(job.output / "evidence-index.json")
        match at(index, ("files",)):
            case dict() as archived if set(archived) == paths | {"coverage.json"}:
                verify_archive(job.output)
            case _:
                raise WorldCheckError("Order archive exact membership differs")
        paths |= {
            "coverage.json",
            "evidence-index.json",
            "evidence.tar.gz",
            "summary.txt",
        }
    _ = bounded_paths(job.output, paths)
    return {
        "successful_pairs": 19,
        "actions": actions,
        "snapshots": 2 * actions + 38,
        "saves": saves,
        "airport_rows": 40,
        "native_crash_boundaries": 3,
        "capture_controls": 2,
        "capture_admission_rejections": 9,
        "semantic_rejections": 13,
        "native_mutation_rejected": True,
        "zero_test_rejected": True,
        "subset_rejected": True,
    }
