# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through runner.py in this directory.
from __future__ import annotations

import json
import os
import subprocess
import time
from collections.abc import Generator
from contextlib import ExitStack, contextmanager
from dataclasses import dataclass
from pathlib import Path
from typing import Literal

from scripts.gameplay_foundations import digest

Role = Literal["sp", "server", "client"]


class NativeError(RuntimeError):
    pass


@dataclass(frozen=True, slots=True)
class Running:
    process: subprocess.Popen[str]
    directory: Path

    def wait_file(self, path: Path, timeout: float = 90) -> None:
        deadline = time.monotonic() + timeout
        while not path.is_file():
            code = self.process.poll()
            if code is not None:
                raise NativeError(
                    f"Native exited {code} before {path}; {self.directory}"
                )
            if time.monotonic() >= deadline:
                raise NativeError(
                    f"Native timeout waiting for {path}; {self.directory}"
                )
            time.sleep(0.05)

    def finish(self) -> None:
        code = self.process.wait(timeout=30)
        if code != 0:
            raise NativeError(f"Native exited {code}; {self.directory}")


@dataclass(frozen=True, slots=True)
class Endpoint:
    role: Role
    port: int = 0
    capture: Literal["armed", "unarmed", "reused"] = "armed"


def game_arguments(source: Path | None, endpoint: Endpoint) -> list[str]:
    match endpoint.role:
        case "sp":
            if source is None:
                raise NativeError("SP source required")
            return ["-vnull:ticks=100000000", "-g", str(source)]
        case "server":
            if source is None or endpoint.port == 0:
                raise NativeError("Server source and loopback port required")
            return ["-D", f"127.0.0.1:{endpoint.port}", "-g", str(source)]
        case "client":
            if source is not None or endpoint.port == 0:
                raise NativeError("Client must join actual loopback session")
            return ["-vnull:ticks=100000000", "-n", f"127.0.0.1:{endpoint.port}#255"]


@contextmanager
def start(
    oracle: Path,
    source: Path | None,
    actions: Path,
    directory: Path,
    endpoint: Endpoint,
) -> Generator[Running, None, None]:
    directory.mkdir(parents=True, exist_ok=False)
    config = directory / "openttd.cfg"
    _ = config.write_text(
        "\n".join(
            (
                "[gui]",
                "autosave = off",
                "autosave_on_exit = false",
                "threaded_saves = false",
                "[misc]",
                "language = english.lng",
                "savegame_format = none",
                "survey_participation = no",
                "[network]",
                "server_game_type = local",
                "server_name = native-order-local",
                "client_name = native-order-client",
                f"server_port = {endpoint.port or 3979}",
                "min_active_clients = 0",
                "autoclean_companies = false",
                "",
            )
        )
    )
    argv = [
        str(oracle),
        "-X",
        "-x",
        "-c",
        str(config),
        "-snull",
        "-mnull",
        "-d",
        "net=3,sl=2",
    ]
    argv += game_arguments(source, endpoint)
    environment = os.environ.copy()
    for key in list(environment):
        if key.startswith("OTTD_"):
            del environment[key]
    environment.update(
        OTTD_ORDER_STATE_PATH=str(directory / "loaded.json"),
        OTTD_WORLD_PATH=str(directory / "saved-world.json"),
        OTTD_WORLD_SCHEMA_PATH=str(directory / "saved-schema.json"),
        OTTD_ORDER_FIXTURE_PATH=str(actions),
        OTTD_ORDER_FIXTURE_DIR=str(directory / "native"),
    )
    if endpoint.role == "client" and endpoint.capture != "unarmed":
        if endpoint.capture == "reused":
            _ = (directory / "received.sav").write_bytes(b"sentinel\n")
        environment["OTTD_ORDER_NETWORK_INPUT_PATH"] = str(directory / "received.sav")
    _ = (directory / "argv.json").write_text(json.dumps(argv, indent=2) + "\n")
    _ = (directory / "environment.json").write_text(
        json.dumps(
            {
                key: value
                for key, value in environment.items()
                if key.startswith("OTTD_")
            },
            indent=2,
        )
        + "\n"
    )
    inputs = [oracle, actions, config]
    if source is not None:
        inputs.append(source)
    before = {str(path): digest(path) for path in inputs}
    _ = (directory / "bindings-before.json").write_text(
        json.dumps(before, indent=2) + "\n"
    )
    began = time.time_ns()
    with ExitStack() as stack:
        stdout = stack.enter_context((directory / "stdout.log").open("w"))
        stderr = stack.enter_context((directory / "stderr.log").open("w"))
        process = subprocess.Popen(
            argv,
            cwd=directory,
            env=environment,
            stdout=stdout,
            stderr=stderr,
            text=True,
            stdin=subprocess.DEVNULL,
        )
        cleanup = "already-exited"
        try:
            yield Running(process, directory)
        finally:
            if process.poll() is None:
                cleanup = "owned-process-terminate"
                process.terminate()
                try:
                    _ = process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    cleanup = "owned-process-kill-after-timeout"
                    process.kill()
                    _ = process.wait(timeout=5)
            _ = (directory / "process.json").write_text(
                json.dumps(
                    {
                        "pid": process.pid,
                        "returncode": process.returncode,
                        "cleanup": cleanup,
                        "started_ns": began,
                        "finished_ns": time.time_ns(),
                    },
                    indent=2,
                )
                + "\n"
            )

    after = {str(path): digest(path) for path in inputs}
    _ = (directory / "bindings-after.json").write_text(
        json.dumps(after, indent=2) + "\n"
    )
    if before != after:
        raise NativeError("Native process input or executable changed")


def execute(oracle: Path, source: Path, actions: Path, directory: Path) -> None:
    with start(oracle, source, actions, directory, Endpoint("sp")) as process:
        process.wait_file(directory / "native/results.json")
        process.finish()
