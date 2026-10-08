# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.10"
# dependencies = []
# ///
"""Self-authored native content and bounded fixture-generation support."""

import json
import re
import shutil
import struct
import subprocess
from pathlib import Path
from typing import Final

from native_contract_protocol import ContractError

ROOT: Final = Path(__file__).resolve().parents[1]
CONTENT: Final = ROOT / "fixtures/content"


def grf_bytes() -> bytes:
    """Wrap explicit pseudo-sprite source with the native v1 GRF container."""
    sprites = [
        bytes.fromhex(line.split("#", 1)[0])
        for line in (CONTENT / "contract-speed.hex").read_text().splitlines()
        if line.split("#", 1)[0].strip()
    ]
    header = struct.pack("<HB I", 4, 255, len(sprites))
    return (
        header
        + b"".join(struct.pack("<HB", len(sprite), 255) + sprite for sprite in sprites)
        + bytes(2)
    )


def prepare_case(directory: Path, parameter: int | None) -> Path:
    """Create a fresh isolated run directory and deterministic native configuration."""
    directory.mkdir(parents=True, exist_ok=False)
    (directory / "ai").mkdir()
    shutil.copytree(CONTENT / "contract-probe", directory / "ai/ContractProbe")
    config = (ROOT / "scripts/reference.cfg").read_text()
    config += "\n[ai]\nai_in_multiplayer = true\n"
    if parameter is not None:
        (directory / "newgrf").mkdir()
        shutil.copyfile(CONTENT / "contract-speed.grf", directory / "newgrf/contract-speed.grf")
        config += f"\n[newgrf]\ncontract-speed.grf = {parameter}\n"
    target = directory / "openttd.cfg"
    target.write_text(config)
    return target


def run_game(oracle: Path, directory: Path, input_save: Path | None) -> None:
    """Run a bounded native case and reject both process and save/load failures."""
    args = [
        str(oracle),
        "-X",
        "-x",
        "-c",
        str(directory / "openttd.cfg"),
        "-vnull:ticks=500",
        "-snull",
        "-mnull",
        "-d",
        "sl=2,grf=1,script=2,desync=2",
    ]
    args += ["-g", str(input_save)] if input_save else ["-g", "-G", "12345"]
    (directory / "commands.json").write_text(json.dumps(args, indent=2) + "\n")
    with (
        (directory / "stdout.log").open("wb") as stdout,
        (directory / "stderr.log").open("wb") as stderr,
    ):
        result = subprocess.run(
            args, cwd=directory, stdout=stdout, stderr=stderr, timeout=30, check=False
        )
    log = (directory / "stderr.log").read_text(errors="replace")
    (directory / "process.json").write_text(
        json.dumps(
            {"returncode": result.returncode, "load_count": log.count("Loading savegame version ")},
            indent=2,
        )
        + "\n"
    )
    if "[grf:0]" in log:
        raise ContractError(f"Native content warning invalidates reference setup: {directory}")
    expected_loads = 2 if input_save else 1
    if (
        result.returncode != 0
        or "[sl:0]" in log
        or log.count("Loading savegame version ") != expected_loads
    ):
        raise ContractError(f"Native load failed or fell back: {directory}")
    output = directory / "save/autosave/exit.sav"
    if (
        not output.exists()
        or output.stat().st_size < 8
        or output.read_bytes()[:8] != b"OTTN\x01j\x00\x00"
    ):
        raise ContractError(f"Missing native uncompressed version362 save: {directory}")


def assert_observer(directory: Path, expected: int, restored: bool) -> None:
    log = (directory / "stderr.log").read_text(errors="replace")
    if f"CONTRACT_SPEED engine=0 speed={expected}\n" not in log:
        raise ContractError(f"Expected native train speed {expected}: {directory}")
    marker = "true" if restored else "false"
    if f"CONTRACT_RESTORED {marker}\n" not in log:
        raise ContractError(f"Expected native script restored={marker}: {directory}")
    company = (
        "CONTRACT_COMPANY name=Contract Probe"
        if restored
        else "CONTRACT_RENAME result=true name=Contract Probe"
    )
    if company + "\n" not in log:
        raise ContractError(f"Expected native company command outcome: {directory}")


def generate(oracle: Path, directory: Path, parameter: int) -> Path:
    prepare_case(directory, parameter)
    (directory / "scripts").mkdir()
    (directory / "scripts/game_start.scr").write_text("start_ai ContractProbe\n")
    run_game(oracle, directory, None)
    assert_observer(directory, parameter, False)
    return directory / "save/autosave/exit.sav"


def check_command_trace(directory: Path) -> None:
    """Compare native dated command records, excluding host wall-clock prefixes."""
    text = (directory / "save/autosave/commands-out.log").read_text()
    records = re.findall(
        r" (cmdf?): ([0-9a-f]+); ([0-9a-f]+); ([0-9a-f]+); ([0-9a-f]+); ([0-9a-f]+); ([0-9A-F]+) \(([^)]+)\)",
        text,
    )
    expected = [
        ("cmd", "000ade1f", "00", "ff", "0000005a", "00000000", "01FF0001000000", "CmdCompanyCtrl"),
        (
            "cmd",
            "000ade1f",
            "04",
            "00",
            "0000003e",
            "00000000",
            "436F6E74726163742050726F626500",
            "CmdRenameCompany",
        ),
    ]
    (directory / "observed-commands.json").write_text(json.dumps(records, indent=2) + "\n")
    if records != expected:
        raise ContractError(f"Native dated command trace differs: {directory}")
