#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Run original-only native content and prejoin protocol baselines.

Run: python3 scripts/native_contract.py [--oracle PATH] [--run-dir FRESH_PATH]
"""

import argparse
import hashlib
import json
import socket
import subprocess
import tempfile
import threading
from dataclasses import asdict
from pathlib import Path

from native_contract_content import (
    CONTENT,
    ROOT,
    assert_observer,
    check_command_trace,
    generate,
    grf_bytes,
    prepare_case,
    run_game,
)
from native_contract_protocol import (
    QUERY,
    ContractError,
    GrfIdentity,
    parse_game_info,
    query,
    wrong_revision,
)


def check(condition: bool, message: str) -> None:
    if not condition:
        raise ContractError(message)


def record_json(path: Path, value: list[str] | dict[str, str | int | bool | list[str]]) -> None:
    path.write_text(json.dumps(value, indent=2) + "\n")


def probe_server(oracle: Path, directory: Path, modded: bool) -> None:
    """Query only a localhost process owned here, then always stop and reap it."""
    prepare_case(directory, 123 if modded else None)
    fixture = ROOT / (
        "fixtures/contracts/modded-v362.sav" if modded else "fixtures/generated-v362.sav"
    )
    name = "Contract-modded" if modded else "Contract-vanilla"
    config = directory / "openttd.cfg"
    config.write_text(
        config.read_text().replace(
            "server_game_type = local", f"server_game_type = local\nserver_name = {name}"
        )
    )
    with socket.socket() as reservation:
        reservation.bind(("127.0.0.1", 0))
        port = reservation.getsockname()[1]
    args = [
        str(oracle),
        "-X",
        "-x",
        "-c",
        str(config),
        "-D",
        f"127.0.0.1:{port}",
        "-g",
        str(fixture),
        "-d",
        "sl=2,net=9,script=2",
    ]
    record_json(directory / "commands.json", args)
    with (
        (directory / "stdout.log").open("wb") as stdout,
        (directory / "stderr.log").open("wb") as stderr,
    ):
        with subprocess.Popen(
            args, cwd=directory, stdin=subprocess.PIPE, stdout=stdout, stderr=subprocess.PIPE
        ) as process:
            ready = threading.Event()
            listening = f"[net:3] Listening on 127.0.0.1:{port} ".encode()
            pipe = process.stderr
            check(pipe is not None, "Native stderr pipe was not created")

            def record_stderr() -> None:
                if pipe is None:
                    return
                for line in pipe:
                    stderr.write(line)
                    stderr.flush()
                    if listening in line:
                        ready.set()

            logger = threading.Thread(target=record_stderr, daemon=True)
            logger.start()
            try:
                check(ready.wait(timeout=15), f"Native server readiness timed out: {directory}")
                check(process.poll() is None, f"Native server exited early: {directory}")
                packet = query(port)
                (directory / "query.bin").write_bytes(QUERY)
                (directory / "response.bin").write_bytes(packet)
                info = parse_game_info(packet)
                (directory / "game-info.json").write_text(json.dumps(asdict(info), indent=2) + "\n")
                check(
                    info.revision == "15.3" and info.name == name, "Unexpected server revision/name"
                )
                check(
                    (info.width, info.height, info.landscape, info.dedicated) == (64, 64, 0, True),
                    "Unexpected server map",
                )
                check(info.calendar_start == 712223, "Unexpected starting date")
                expected_grfs = (
                    (
                        GrfIdentity(
                            "52555354",
                            hashlib.md5(grf_bytes(), usedforsecurity=False).hexdigest(),
                            "Contract Speed",
                        ),
                    )
                    if modded
                    else ()
                )
                check(
                    info.grfs == expected_grfs,
                    "Native advertised content differs from exact requested content",
                )
                request, response = wrong_revision(port)
                (directory / "wrong-revision-query.bin").write_bytes(request)
                (directory / "wrong-revision-response.bin").write_bytes(response)
                for bad in (packet[:-1], packet + b"\x00", packet[:3] + b"\x08" + packet[4:]):
                    try:
                        parse_game_info(bad)
                    except ContractError:
                        continue
                    raise ContractError("Malformed/unsupported packet unexpectedly accepted")
                record_json(
                    directory / "controls.json",
                    {
                        "wrong_revision": "rejected before authentication",
                        "truncated": "rejected",
                        "trailing": "rejected",
                        "schema8": "rejected",
                    },
                )
            finally:
                if process.poll() is None:
                    try:
                        if process.stdin is not None:
                            process.stdin.write(b"quit\n")
                            process.stdin.flush()
                        process.wait(timeout=5)
                    except subprocess.TimeoutExpired:
                        process.kill()
                        process.wait(timeout=5)
                logger.join(timeout=5)
                check(not logger.is_alive(), "Native stderr recorder did not finish")
    log = (directory / "stderr.log").read_text(errors="replace")
    check(process.returncode == 0, f"Native server did not exit cleanly: {directory}")
    check(
        "[sl:0]" not in log and log.count("Loading savegame version ") == 2,
        "Server load failed/fell back",
    )


def run(oracle: Path, artifacts: Path) -> None:
    check(oracle.is_file(), f"Missing native oracle: {oracle}")
    check(
        (CONTENT / "contract-speed.grf").read_bytes() == grf_bytes(),
        "GRF binary differs from source",
    )
    version = subprocess.run([str(oracle), "-h"], capture_output=True, timeout=10, check=True)
    (artifacts / "oracle-help.log").write_bytes(version.stdout + version.stderr)
    check(
        (version.stdout + version.stderr).startswith(b"OpenTTD 15.3\n"), "Oracle is not OpenTTD15.3"
    )
    generated = generate(oracle, artifacts / "parameter123", 123)
    generate(oracle, artifacts / "parameter77", 77)
    # The same assertion must reject a different, actually observed parameter effect.
    try:
        assert_observer(artifacts / "parameter77", 123, False)
    except ContractError as error:
        (artifacts / "parameter-mismatch.log").write_text(str(error) + "\n")
    else:
        raise ContractError("Parameter mismatch did not fail")
    for label, fixture in [
        ("fresh-reload", generated),
        ("fixture-reload", ROOT / "fixtures/contracts/modded-v362.sav"),
    ]:
        directory = artifacts / label
        prepare_case(directory, 123)
        run_game(oracle, directory, fixture)
        assert_observer(directory, 123, True)
    check_command_trace(artifacts / "parameter123")
    check_command_trace(artifacts / "parameter77")
    missing = artifacts / "missing-content"
    prepare_case(missing, None)
    try:
        run_game(oracle, missing, ROOT / "fixtures/contracts/modded-v362.sav")
    except ContractError as error:
        (missing / "expected-rejection.log").write_text(str(error) + "\n")
        log = (missing / "stderr.log").read_text(errors="replace")
        check(
            "NewGRF 52555354 (contract-speed.grf) not found" in log,
            "Missing-content failure had an unrelated cause",
        )
    else:
        raise ContractError("Missing NewGRF was silently accepted")
    probe_server(oracle, artifacts / "protocol-vanilla", False)
    probe_server(oracle, artifacts / "protocol-modded", True)
    record_json(
        artifacts / "summary.json",
        {
            "status": "pass",
            "scope": "original_only",
            "savegame_version": 362,
            "game_info_schema": 7,
            "cases": [
                "parameter123",
                "parameter77",
                "parameter-mismatch",
                "fresh-reload",
                "fixture-reload",
                "native-command-record",
                "missing-content",
                "protocol-vanilla",
                "protocol-modded",
                "wrong-revision",
                "malformed-packet",
                "unsupported-schema",
            ],
        },
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle", type=Path, default=ROOT / ".reference/build/openttd")
    parser.add_argument("--run-dir", type=Path)
    args = parser.parse_args()
    if args.run_dir:
        artifacts = args.run_dir.resolve()
        artifacts.mkdir(parents=True, exist_ok=False)
    else:
        evidence = ROOT / ".omo/evidence"
        evidence.mkdir(parents=True, exist_ok=True)
        artifacts = Path(tempfile.mkdtemp(prefix="native-contract-", dir=evidence))
    print(f"Artifacts: {artifacts}", flush=True)
    try:
        run(args.oracle.resolve(), artifacts)
    except (ContractError, OSError, subprocess.SubprocessError) as error:
        (artifacts / "failure.log").write_text(f"{type(error).__name__}: {error}\n")
        print(f"FAIL: {error}", flush=True)
        return 1
    print(
        "PASS: original-only native content, command recording and prejoin game-info contract",
        flush=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
