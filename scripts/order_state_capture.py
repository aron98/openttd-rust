# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through scripts/check-order-state.py.
from __future__ import annotations

from configparser import ConfigParser
from copy import deepcopy
from pathlib import Path

from scripts.context_ci_support import exact
from scripts.grf_control_evidence import sequence, text
from scripts.order_state_evidence import fields
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


def role_paths(value: Json) -> list[tuple[str | int, ...]]:
    fields(
        value,
        {"schema_version", "case", "initial", "final", "role", "final_role", "actions"},
    )
    result: list[tuple[str | int, ...]] = [("role",)]
    for index, row in enumerate(sequence(at(value, ("actions",)))):
        fields(row, {"index", "input", "before", "after", "result", "role"})
        if at(row, ("input", "op")) == "save":
            fields(at(row, ("result",)), {"path", "before", "after", "role"})
            result.append(("actions", index, "result", "role"))
        result.append(("actions", index, "role"))
    result.append(("final_role",))
    return result


def checked_trace(value: Json, name: str) -> list[tuple[tuple[str | int, ...], Json]]:
    result: list[tuple[tuple[str | int, ...], Json]] = []
    previous = 1
    expected: list[Json] = [
        {"id": 1, "name": name, "company": 255},
        {"id": 2, "name": name + " #1", "company": 255},
    ]
    for path in role_paths(value):
        role = at(value, path)
        fields(role, {"clients", "networking", "server", "dedicated", "own_client_id"})
        core: Json = {
            key: at(role, (key,))
            for key in ("networking", "server", "dedicated", "own_client_id")
        }
        if not exact(
            core,
            {
                "networking": True,
                "server": False,
                "dedicated": False,
                "own_client_id": 2,
            },
        ):
            raise WorldCheckError("Capture actual client role differs")
        clients = sequence(at(role, ("clients",)))
        if len(clients) not in (1, 2) or not exact(clients, expected[: len(clients)]):
            raise WorldCheckError("Capture client identity/name/company prefix differs")
        if len(clients) < previous:
            raise WorldCheckError("Capture client arrival is not monotonic")
        previous = len(clients)
        result.append(((*path, "clients"), clients))
    return result


def compare_capture(left: Json, right: Json, name: str) -> list[Json]:
    before, after = deepcopy(left), deepcopy(right)
    traces = [checked_trace(value, name) for value in (before, after)]
    differences: list[Json] = []
    if [path for path, _ in traces[0]] != [path for path, _ in traces[1]]:
        raise WorldCheckError("Capture recognized role locations differ")
    for (path, first), (_, second) in zip(traces[0], traces[1], strict=True):
        if not exact(first, second):
            differences.append({"path": list(path), "armed": first, "unarmed": second})
        for value in (before, after):
            match at(value, path[:-1]):
                case dict() as role:
                    _ = role.pop("clients")
                case _:
                    raise WorldCheckError("Missing exact capture role object")
    if not exact(before, after):
        raise WorldCheckError(
            "Capture state changed outside recognized client arrivals"
        )
    return differences


def endpoint(case: Path) -> str:
    server = case / "server"
    client = case / "client/original"
    argv = [text(value) for value in sequence(read_json(client / "argv.json"))]
    server_argv = [text(value) for value in sequence(read_json(server / "argv.json"))]
    address = server_argv[server_argv.index("-D") + 1]
    if (
        not address.startswith("127.0.0.1:")
        or argv[argv.index("-n") + 1] != address + "#255"
    ):
        raise WorldCheckError("Capture client did not join its actual loopback server")
    names: list[str] = []
    for directory, command in ((server, server_argv), (client, argv)):
        config = directory / "openttd.cfg"
        if command[command.index("-c") + 1] != str(config):
            raise WorldCheckError("Capture startup config identity differs")
        parser = ConfigParser()
        _ = parser.read(config)
        names.append(parser.get("network", "client_name"))
    if names != ["native-order-client", "native-order-client"]:
        raise WorldCheckError("Capture fixture configured names differ")
    role = at(read_json(server / "loaded.json"), ("role",))
    if not exact(
        {
            key: at(role, (key,))
            for key in ("networking", "server", "dedicated", "own_client_id")
        },
        {"networking": True, "server": True, "dedicated": True, "own_client_id": 1},
    ):
        raise WorldCheckError(
            "Capture server did not start as the genuine dedicated host"
        )
    return names[0]


def capture_pair(output: Path) -> Json:
    armed = output / "network/single"
    unarmed = output / "capture/unarmed"
    name = endpoint(armed)
    if name != endpoint(unarmed):
        raise WorldCheckError("Capture fixture names differ")
    left = read_json(armed / "client/original/native/results.json")
    right = read_json(unarmed / "client/original/native/results.json")
    differences = compare_capture(left, right, name)
    if not exact(
        read_json(armed / "client/original/saved-world.json"),
        read_json(unarmed / "client/original/saved-world.json"),
    ):
        raise WorldCheckError("Unarmed capture changed complete saved state")
    receipt: Json = {
        "boundary": "native host client-info packet arrival only",
        "differences": differences,
        "armed_trace": [
            {"path": list(path), "clients": clients}
            for path, clients in checked_trace(left, name)
        ],
        "unarmed_trace": [
            {"path": list(path), "clients": clients}
            for path, clients in checked_trace(right, name)
        ],
        "all_other_fields_equal": True,
        "saved_world_equal": True,
    }
    return receipt


def record_capture(output: Path) -> None:
    write_json(output / "capture/client-arrival-boundary.json", capture_pair(output))
