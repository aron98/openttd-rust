# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through scripts/check-order-state.py.
from __future__ import annotations

import shutil
from copy import deepcopy
from typing import Final

from scripts.context_ci_support import exact
from scripts.gameplay_foundations import digest, require_test
from scripts.grf_control_evidence import sequence, text
from scripts.order_state_capture import compare_capture
from scripts.order_state_fixtures import OrderRun
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    read_json,
    replace,
    run,
    write_json,
)


def execution_controls(job: OrderRun) -> None:
    output = job.output / "controls"
    output.mkdir()
    runner = text(at(job.manifest, ("runner", "executable")))
    selector = text(at(job.manifest, ("case_test",)))
    result = run(
        [runner, "--exact", "order_state_nonexistent", "--ignored"], output / "zero"
    )
    try:
        require_test(result.stdout, selector)
    except WorldCheckError:
        if "0 passed; 0 failed; 0 ignored" not in result.stdout:
            raise WorldCheckError("Zero control did not execute zero tests") from None
    else:
        raise WorldCheckError("Zero tests admitted")
    descriptor = deepcopy(job.descriptor("network/single/client"))
    replace(descriptor, ("actions",), [at(descriptor, ("actions", 0))])
    write_json(output / "subset.json", descriptor)
    source = job.output / "network/single/client/original/received.sav"
    result = run(
        [
            "env",
            f"ORDER_CASE_INPUT={source}",
            f"ORDER_CASE_ACTIONS={output / 'subset.json'}",
            f"ORDER_CASE_OUTPUT={output / 'subset'}",
            runner,
            "--exact",
            selector,
            "--ignored",
            "--nocapture",
        ],
        output / "subset-command",
    )
    require_test(result.stdout, selector)
    actual = at(read_json(output / "subset/results.json"), ("actions",))
    if len(sequence(actual)) != 1:
        raise WorldCheckError("Subset control did not execute one action")
    write_json(
        output / "expected-actions.json",
        at(job.descriptor("network/single/client"), ("actions",)),
    )
    write_json(output / "actual-actions.json", [at(actual, (0, "input"))])
    _ = run(
        [
            job.cli(),
            "compare",
            str(output / "expected-actions.json"),
            str(output / "actual-actions.json"),
        ],
        output / "subset-rejected",
        expected=1,
    )
    write_json(
        output / "execution.json",
        {"zero_tests_rejected": True, "successful_subset_test_rejected": True},
    )


def semantic_controls(job: OrderRun) -> None:
    shared = read_json(
        job.output / "paired/sp/cases/shared-three-members/native-state.json"
    )
    remote = read_json(
        job.output / "paired/sp/cases/remote-owned-nearest-and-clone/native-state.json"
    )
    loaded = read_json(job.output / "paired/network/multi/client/native-state.json")
    prefix: tuple[str | int, ...] = ("actions", 1, "after")
    controls: tuple[tuple[str, Json, tuple[str | int, ...], Json], ...] = (
        ("dummy", shared, (*prefix, "lists", 0, "orders", 0, "type"), 2),
        ("wait", shared, (*prefix, "vehicles", 0, "current_order", "wait_time"), 999),
        (
            "travel",
            shared,
            (*prefix, "vehicles", 0, "current_order", "travel_time"),
            999,
        ),
        ("flags", shared, (*prefix, "lists", 0, "orders", 0, "flags"), 255),
        ("duration", shared, (*prefix, "lists", 0, "total_duration"), 999),
        ("members", shared, (*prefix, "lists", 0, "members"), []),
        ("user", remote, (*prefix, "backups", 0, "user"), 999),
        ("id", remote, (*prefix, "backups", 0, "id"), 254),
        ("clone", remote, (*prefix, "backups", 0, "clone"), 999),
        ("allocator", shared, ("final", "backup_pool", "first_free"), 255),
        ("actions", shared, ("actions",), []),
        ("object-index", loaded, ("initial", "backups", 1, "id"), 1),
        ("physical-slot", loaded, ("initial", "backups", 1, "pool_slot"), 0),
    )
    for name, original, path, value in controls:
        directory = job.output / "controls/semantic" / name
        directory.mkdir(parents=True)
        if exact(at(original, path), value):
            raise WorldCheckError("Semantic control is a no-op")
        changed = deepcopy(original)
        replace(changed, path, value)
        write_json(directory / "original.json", original)
        write_json(directory / "changed.json", changed)
        _ = run(
            [
                job.cli(),
                "compare",
                str(directory / "original.json"),
                str(directory / "changed.json"),
            ],
            directory / "command",
            expected=1,
        )
        write_json(directory / "mutation.json", {"path": list(path), "value": value})


def capture_controls(job: OrderRun) -> None:
    left = read_json(job.output / "network/single/client/original/native/results.json")
    right = read_json(
        job.output / "capture/unarmed/client/original/native/results.json"
    )
    for name, path, value in CAPTURE_MUTATIONS:
        directory = job.output / "controls/capture" / name
        directory.mkdir(parents=True)
        changed = deepcopy(right)
        replace(changed, path, value)
        write_json(directory / "changed.json", changed)
        try:
            _ = compare_capture(left, changed, "native-order-client")
        except WorldCheckError as error:
            write_json(
                directory / "rejected.json",
                {"rejected": True, "path": list(path), "diagnostic": str(error)},
            )
        else:
            raise WorldCheckError("Capture admission accepted an unauthorized mutation")


CAPTURE_MUTATIONS: Final[tuple[tuple[str, tuple[str | int, ...], Json], ...]] = (
    ("networking", ("role", "networking"), False),
    ("server", ("role", "server"), True),
    ("dedicated", ("role", "dedicated"), True),
    ("own-id", ("role", "own_client_id"), 3),
    ("client-id", ("role", "clients", 0, "id"), 7),
    ("client-name", ("role", "clients", 0, "name"), "changed"),
    ("client-company", ("role", "clients", 0, "company"), 0),
    ("unauthorized-location", ("clients",), []),
    ("gameplay", ("initial", "backups", 0, "user"), 999),
)


def native_admission(job: OrderRun) -> None:
    directory = job.output / "controls/native"
    directory.mkdir()
    binary = directory / "openttd"
    _ = shutil.copy2(job.oracle, binary)
    before = digest(binary)
    original_stamp = (
        (job.oracle.parent / "replay-build.sha256").read_text().splitlines()
    )
    stamp = directory / "replay-build.sha256"
    _ = stamp.write_text(
        before + "  " + str(binary) + "\n" + "\n".join(original_stamp[1:]) + "\n"
    )
    argv = [
        "cmake",
        f"-DORACLE={binary}",
        "-P",
        str(job.root / "scripts/check-replay-build.cmake"),
    ]
    _ = run(argv, directory / "before")
    binary.chmod(0o755)
    altered = bytearray(binary.read_bytes())
    altered[-1] ^= 1
    _ = binary.write_bytes(altered)
    binary.chmod(0o555)
    result = run(argv, directory / "rejected", expected=1)
    if (
        "Stale native replay binary/source" not in result.stderr
        or digest(binary) == before
    ):
        raise WorldCheckError(
            "Changed native executable was not rejected by actual freshness gate"
        )
    write_json(
        directory / "mutation.json",
        {
            "before_sha256": before,
            "after_sha256": digest(binary),
            "byte_offset": len(altered) - 1,
            "xor": 1,
        },
    )
