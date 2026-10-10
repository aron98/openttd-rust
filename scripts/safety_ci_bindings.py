# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through scripts/check-grf-safety-ci.py.
from __future__ import annotations

from collections.abc import Mapping
from pathlib import Path

from scripts.context_ci_support import exact, native_bindings
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import sequence, text
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


def command_fields(argv: list[str]) -> Mapping[str, str]:
    if len(argv) < 4 or argv[0] != "cmake" or argv[-2] != "-P":
        raise WorldCheckError("Safety guard did not invoke recorded CMake script")
    fields: dict[str, str] = {}
    for value in argv[1:-2]:
        name, separator, raw = value.partition("=")
        if not separator or not name.startswith("-D") or name[2:] in fields:
            raise WorldCheckError("Unexpected safety guard argument")
        fields[name[2:]] = raw
    return fields


def guard_binding(
    root: Path, output: Path, oracle: Path, name: str, expected: Json
) -> Json:
    directory = output / "guards" / name
    receipt = read_json(directory / "receipt.json")
    if not exact(receipt, expected):
        raise WorldCheckError("Safety guard result changed")
    argv = [text(v) for v in sequence(read_json(directory / "argv.json"))]
    fields = command_fields(argv)
    environment = read_json(directory / "environment.json")
    process = read_json(directory / "process.json")
    if not exact(at(process, ("returncode",)), at(receipt, ("exit",))):
        raise WorldCheckError("Safety guard process result differs")
    native, required, run_dir, runner = guard_shape(root, directory, name, receipt)
    if (
        set(fields) != required
        or fields["ORACLE"] != str(oracle)
        or fields["RUN_DIR"] != str(run_dir)
        or argv[-1] != str(runner)
    ):
        raise WorldCheckError("Safety guard native/script identity differs")
    hashes = guard_inputs(root, directory, name, fields, runner)
    guard_environment(directory, oracle, name, environment)
    if native and not (directory / "native/stderr.log").exists():
        raise WorldCheckError("Missing actual native guard process log")
    if (
        name == "duplicate"
        and (directory / "safety.json").read_bytes() != b"sentinel\n"
    ):
        raise WorldCheckError("Duplicate safety output was overwritten")
    return {
        "argv": list(argv),
        "environment": environment,
        "process": process,
        "native_invoked": native,
        "native_sha256": digest(oracle),
        "inputs": hashes,
    }


def guard_shape(
    root: Path, directory: Path, name: str, receipt: Json
) -> tuple[bool, set[str], Path, Path]:
    match receipt:
        case {"phase": "menu", "exit": 0, "observation_exists": False}:
            native = True
            required = {"ORACLE", "RUN_DIR", "CONFIG"}
            run_dir = directory
            runner = directory / "menu.cmake"
        case {"early": bool() as early, "exit": int()}:
            native = not early
            required = {"ORACLE", "RUN_DIR", "CONFIG", "INPUT", "MANIFEST", "TICKS"}
            run_dir = directory / "native"
            runner = (
                root
                / "scripts"
                / (
                    "check-grf-safety-reference.cmake"
                    if early
                    else "run-reference.cmake"
                )
            )
            if name == "stale":
                runner = directory / "stale/scripts/check-grf-safety-reference.cmake"
        case _:
            raise WorldCheckError("Unknown safety guard receipt shape")
    return native, required, run_dir, runner


def guard_inputs(
    root: Path, directory: Path, name: str, fields: Mapping[str, str], runner: Path
) -> Json:
    config = root / "scripts/reference.cfg"
    if fields["CONFIG"] != str(config):
        raise WorldCheckError("Safety guard config differs")
    hashes: dict[str, Json] = {"config": digest(config), "script": digest(runner)}
    if "INPUT" in fields:
        expected_input = (
            "GENERATE"
            if name == "nonsave"
            else str(root / "fixtures/replay/clear-v362.sav")
        )
        if (
            fields["INPUT"] != expected_input
            or fields["MANIFEST"] != str(directory / "manifest.json")
            or fields["TICKS"] != "1"
        ):
            raise WorldCheckError("Safety guard save/manifest/phase differs")
        if expected_input != "GENERATE":
            hashes["input"] = digest(Path(expected_input))
    manifest = sequence(read_json(directory / "manifest.json"))
    hashes["manifest"] = digest(directory / "manifest.json")
    for case in manifest:
        path = Path(text(at(case, ("path",))))
        if path != root / "fixtures/content/contract-speed.grf":
            raise WorldCheckError("Safety guard GRF source changed")
        hashes["grf"] = digest(path)
    return hashes


def guard_environment(
    directory: Path, oracle: Path, name: str, environment: Json
) -> None:
    match environment:
        case dict() as values if set(values) == {
            "OTTD_GRF_ORACLE",
            "OTTD_GRF_SAFETY_INPUT",
            "OTTD_GRF_SAFETY_OUTPUT",
            "OTTD_REPLAY_PATH",
            "OTTD_GRF_SAFETY_CASE",
            "OTTD_GRF_CONTROL_CASE",
        }:
            if values["OTTD_GRF_ORACLE"] != str(oracle):
                raise WorldCheckError("Guard environment oracle differs")
        case _:
            raise WorldCheckError("Guard environment whitelist changed")
    expected_environment: dict[str, Json] = dict.fromkeys(values)
    expected_environment["OTTD_GRF_ORACLE"] = str(oracle)
    match name:
        case "duplicate" | "missing-input" | "missing-output" | "menu":
            if name != "missing-input":
                expected_environment["OTTD_GRF_SAFETY_INPUT"] = str(
                    directory / "manifest.json"
                )
            if name != "missing-output":
                expected_environment["OTTD_GRF_SAFETY_OUTPUT"] = str(
                    directory / "safety.json"
                )
        case "replay":
            expected_environment["OTTD_REPLAY_PATH"] = "forbidden"
        case "subset":
            expected_environment["OTTD_GRF_SAFETY_CASE"] = "only-one"
        case "reuse" | "nonsave" | "stale" | "unarmed":
            pass
        case _:
            raise WorldCheckError("Unknown safety guard identity")
    if not exact(environment, expected_environment):
        raise WorldCheckError("Safety guard effective environment differs")


def bind_safety(root: Path, output: Path, oracle: Path, layout: Json) -> None:
    native_bindings(root, output, oracle)
    positive = read_json(output / "native-bindings.json")
    match positive:
        case dict() if set(positive) == {"results/native/invocation.txt"}:
            pass
        case _:
            raise WorldCheckError("Unexpected positive safety invocation set")
    guards = [text(v) for v in sequence(at(layout, ("guards",)))]
    if len(guards) != 10 or len(set(guards)) != 10:
        raise WorldCheckError("Incomplete safety guard set")
    rows = {
        name: guard_binding(
            root, output, oracle, name, at(layout, ("guard_receipts", name))
        )
        for name in guards
    }
    if sum(at(row, ("native_invoked",)) is True for row in rows.values()) != 5:
        raise WorldCheckError("Safety native guard invocation count changed")
    write_json(
        output / "native-invocations.json",
        {"matrix": positive, "guards": rows, "native_invocations": 6},
    )
