# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through scripts/check-order-state.py.
from __future__ import annotations

import socket
from pathlib import Path
from typing import Literal, Protocol

from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import text
from scripts.order_state_capture import record_capture
from scripts.order_state_fixtures import OrderRun
from scripts.order_state_processes import Endpoint, Running, start
from scripts.world_check_support import WorldCheckError, at, read_json, run, write_json


class BoundIPv4Socket(Protocol):
    def getsockname(self) -> tuple[str, int]: ...


def ipv4_port(listener: BoundIPv4Socket) -> int:
    return listener.getsockname()[1]


def free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
        listener.bind(("127.0.0.1", 0))
        return ipv4_port(listener)


def network(
    job: OrderRun,
    name: str,
    *,
    mode: Literal["armed", "unarmed", "reused", "boundary"] = "armed",
) -> None:
    directory = job.output / name
    directory.mkdir(parents=True)
    count = "multi" if name.endswith("multi") or mode == "boundary" else "single"
    server_plan = directory / "server.json"
    client_plan = directory / "client.json"
    write_json(server_plan, job.descriptor(f"network/{count}/server"))
    client_descriptor = job.descriptor(
        "boundary/client" if mode == "boundary" else f"network/{count}/client"
    )
    write_json(client_plan, client_descriptor)
    source = job.output / "sp/setup/shared-three-members/original/native/initial.sav"
    server_directory = directory / "server"
    case = directory / "client"
    case.mkdir()
    write_json(case / "actions.json", client_descriptor)
    port = free_port()
    capture: Literal["armed", "unarmed", "reused"] = "armed"
    if mode in ("unarmed", "reused"):
        capture = mode
    with start(
        job.oracle, source, server_plan, server_directory, Endpoint("server", port)
    ) as server:
        server.wait_file(server_directory / "native/server-live.sav")
        with start(
            job.oracle,
            None,
            client_plan,
            case / "original",
            Endpoint("client", port, capture),
        ) as client:
            wait_client(client, mode)
        _ = (server_directory / "native/clientdone.ready").write_text(
            "actual client observation completed\n"
        )
        server.wait_file(server_directory / "native/results.json")
    write_json(
        directory / "identity.json",
        {
            "source": str(source),
            "source_sha256": digest(source),
            "oracle_sha256": digest(job.oracle),
            "port": port,
            "mode": mode,
        },
    )
    match mode:
        case "armed":
            job.inputs(case / "original/received.sav", case)
            job.pair(name + "/client")
        case "boundary":
            boundary(job, case / "original/received.sav", case)
        case "unarmed":
            if (case / "original/received.sav").exists():
                raise WorldCheckError("Unarmed network capture wrote an input")
            record_capture(job.output)
        case "reused":
            pass


def wait_client(
    client: Running, mode: Literal["armed", "unarmed", "reused", "boundary"]
) -> None:
    match mode:
        case "reused":
            if (
                client.process.wait(timeout=90) != 1
                or (client.directory / "received.sav").read_bytes() != b"sentinel\n"
                or (client.directory / "loaded.json").exists()
            ):
                raise WorldCheckError(
                    "Reused native capture was not refused before load"
                )
            if (
                "input capture already exists"
                not in (client.directory / "stderr.log").read_text()
            ):
                raise WorldCheckError("Reused capture diagnostic differs")
        case "boundary":
            if client.process.wait(timeout=90) != 2:
                raise WorldCheckError("Original client assertion boundary missing")
        case "armed" | "unarmed":
            client.wait_file(client.directory / "native/results.json")


def boundary(job: OrderRun, source: Path, case: Path) -> None:
    before = digest(source)
    logs = list((case / "original").glob("crash*.json.log"))
    if len(logs) != 1 or "pn == Tpool->Get(Pool::GetRawIndex(pn->index))" not in text(
        at(read_json(logs[0]), ("crash", "reason"))
    ):
        raise WorldCheckError("Original backup invariant assertion missing")
    renamed: dict[str, str] = {}
    for path in sorted((case / "original").glob("crash*")):
        suffix = path.name.removeprefix(path.name.split(".", 1)[0])
        target = path.with_name("crash" + suffix)
        renamed[path.name] = target.name
        _ = path.rename(target)
    write_json(case / "crash-paths.json", dict(renamed))
    result = run(
        [
            "env",
            f"ORDER_CASE_INPUT={source}",
            f"ORDER_CASE_ACTIONS={case / 'actions.json'}",
            f"ORDER_CASE_OUTPUT={case / 'rust'}",
            text(at(job.manifest, ("runner", "executable"))),
            "--exact",
            text(at(job.manifest, ("case_test",))),
            "--ignored",
            "--nocapture",
        ],
        case / "command",
        expected=101,
    )
    if (
        "1 failed; 0 ignored" not in result.stdout
        or "NativeBackupIndex { slot: 1, index: 0 }" not in result.stderr
        or (case / "rust/results.json").exists()
        or digest(source) != before
    ):
        raise WorldCheckError("Rust native-invariant boundary differs")
    write_json(
        case / "boundary.json",
        {
            "parity_success": False,
            "native_exit": 2,
            "rust_exit": 101,
            "source": str(source),
            "source_sha256": before,
            "slot": 1,
            "index": 0,
        },
    )


def roles_and_boundaries(job: OrderRun) -> None:
    for role in ("sp", "server"):
        case = job.output / "role" / role
        case.mkdir(parents=True)
        write_json(case / "actions.json", job.descriptor("role/" + role))
        source = job.output / "network/single/server/native/server-live.sav"
        job.inputs(source, case)
        endpoint = Endpoint(role, free_port() if role == "server" else 0)
        with start(
            job.oracle, source, case / "actions.json", case / "original", endpoint
        ) as running:
            running.wait_file(case / "original/native/results.json")
            running.finish()
        job.pair("role/" + role)
        failed = job.output / "boundary" / role
        failed.mkdir(parents=True)
        write_json(failed / "actions.json", job.descriptor("boundary/" + role))
        source = job.output / "network/multi/server/native/server-live.sav"
        with start(
            job.oracle, source, failed / "actions.json", failed / "original", endpoint
        ) as running:
            if running.process.wait(timeout=30) != 2:
                raise WorldCheckError("Original afterload assertion boundary missing")
        boundary(job, source, failed)
    network(job, "boundary/client", mode="boundary")
