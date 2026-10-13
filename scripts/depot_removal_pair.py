from __future__ import annotations

from copy import deepcopy
from pathlib import Path

from scripts.context_ci_support import exact
from scripts.depot_removal_run import SELECTOR, RemovalRun, member
from scripts.gameplay_foundations import digest, require_test
from scripts.grf_control_evidence import sequence, text
from scripts.order_state_pair import object_at, shared_projection
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    read_json,
    run,
    write_json,
)


def projection(native: Json, descriptor: Json) -> Json:
    role: Json = {
        "clients": [],
        "dedicated": False,
        "networking": False,
        "own_client_id": 0,
        "server": False,
    }
    fields = object_at(native)
    if (
        set(fields)
        != {
            "schema_version",
            "case",
            "role",
            "initial",
            "actions",
            "final",
            "final_role",
        }
        or not exact(fields["role"], role)
        or not exact(fields["final_role"], role)
    ):
        raise WorldCheckError("Depot removal native SP role/schema differs")
    actions = sequence(fields["actions"])
    expected = sequence(at(descriptor, ("actions",)))
    if (
        not expected
        or len(actions) != len(expected)
        or fields["schema_version"] != 1
        or not exact(fields["case"], at(descriptor, ("case",)))
    ):
        raise WorldCheckError("Depot removal action membership differs")
    for index, (row, action) in enumerate(zip(actions, expected, strict=True)):
        entry = object_at(row)
        if (
            set(entry) != {"index", "input", "before", "result", "after", "role"}
            or entry["index"] != index
            or not exact(entry["input"], action)
            or not exact(entry["role"], role)
        ):
            raise WorldCheckError("Depot removal action identity differs")
        if at(action, ("op",)) == "save" and not exact(
            at(entry["result"], ("role",)), role
        ):
            raise WorldCheckError("Depot save role differs")
    result = deepcopy(shared_projection(native, native=True))
    for row in sequence(at(result, ("actions",))):
        if at(row, ("input", "op")) != "command":
            continue
        payload = object_at(at(row, ("result",)))
        command_metadata(payload)
    return result


def command_metadata(payload: dict[str, Json]) -> None:
    if set(payload) != {"receipt", "native_metadata"}:
        raise WorldCheckError("Depot command observation schema differs")
    metadata = object_at(payload["native_metadata"])
    receipt = object_at(payload["receipt"])
    phases = {
        phase for phase in ("test", "exec", "result") if receipt.get(phase) is not None
    }
    if set(metadata) != phases:
        raise WorldCheckError("Depot command phase metadata differs")
    for value in metadata.values():
        phase = object_at(value)
        if set(phase) != {"error_id", "extra_error_id", "owner"}:
            raise WorldCheckError("Unexpected native command metadata")
        for key, maximum in (
            ("error_id", 65535),
            ("extra_error_id", 65535),
            ("owner", 255),
        ):
            item = phase[key]
            if type(item) is not int or not 0 <= item <= maximum:
                raise WorldCheckError("Malformed native command metadata")
    del payload["native_metadata"]


def pair(job: RemovalRun, name: str) -> None:
    case = member(job.output / "results", name)
    inputs = read_json(case / "inputs.json")
    source = Path(text(at(inputs, ("source",))))
    descriptor = read_json(case / "actions.json")
    cli = job.executable("cli")
    result = run(
        [
            "env",
            f"DEPOT_REMOVAL_INPUT={source}",
            f"DEPOT_REMOVAL_ACTIONS={case}/actions.json",
            f"DEPOT_REMOVAL_OUTPUT={case}/rust",
            job.executable("runner"),
            "--exact",
            SELECTOR,
            "--ignored",
            "--nocapture",
        ],
        case / "rust-command",
    )
    require_test(result.stdout, SELECTOR)
    native = read_json(case / "original/native/results.json")
    projected = projection(native, descriptor)
    write_json(case / "native-state.json", projected)
    _ = run(
        [
            cli,
            "compare",
            str(case / "native-state.json"),
            str(case / "rust/results.json"),
        ],
        case / "live-compare",
    )
    saves = [
        text(at(action, ("label",)))
        for action in sequence(at(descriptor, ("actions",)))
        if at(action, ("op",)) == "save"
    ]
    if not saves or len(saves) != len(set(saves)):
        raise WorldCheckError("Depot saved checkpoint membership differs")
    for directory in (case / "original/native", case / "rust"):
        if {path.stem for path in directory.glob("*.sav")} != set(saves):
            raise WorldCheckError("Depot save omitted or added")
    for label in saves:
        for kind, directory in (
            ("native", case / "original/native"),
            ("rust", case / "rust"),
        ):
            exported = run(
                [cli, "world", str(directory / f"{label}.sav"), "--view", "saved"],
                case / f"{label}-{kind}-export",
            )
            _ = (case / f"{label}-{kind}.json").write_text(exported.stdout)
        _ = run(
            [
                cli,
                "compare",
                str(case / f"{label}-native.json"),
                str(case / f"{label}-rust.json"),
            ],
            case / f"{label}-compare",
        )
    _ = run(
        [
            cli,
            "compare",
            str(case / "original/saved-world.json"),
            str(case / f"{saves[-1]}-rust.json"),
        ],
        case / "serializer-compare",
    )
    if digest(source) != at(inputs, ("source_sha256",)) or digest(
        case / "actions.json"
    ) != at(inputs, ("descriptor_sha256",)):
        raise WorldCheckError("Depot removal source input changed")
    print(f"PASS original depot removal {name}", flush=True)
