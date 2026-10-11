# SPDX-License-Identifier: GPL-2.0-only
"""Arrays session observation validation."""

import re
from pathlib import Path

from scripts.script_vm_arrays_spec import (
    SOURCE,
    argv,
    capture,
    declarations,
    working_directory,
)
from scripts.world_check_support import ROOT, WorldCheckError, read_json


def validate_session(directory: Path, origin: Path, name: str) -> None:
    output = directory / "arrays/sessions" / name
    if read_json(output / "argv.json") != argv(origin, name) or read_json(
        output / "cwd.json"
    ) != str(working_directory(origin, name)):
        raise WorldCheckError("Array session invocation differs")
    row = declarations()[name]
    if read_json(output / "process.json") != {
        "returncode": row.status,
        "expected": row.status,
    }:
        raise WorldCheckError("Array session process failed")
    if (output / "stderr.log").read_text() != row.stderr:
        raise WorldCheckError("Array session stderr differs")
    observed = (output / "stdout.log").read_text()
    expected = (ROOT / SOURCE / capture(name)).read_text()
    if comparable(name, observed) != comparable(name, expected):
        raise ArrayObservationMismatchError("Array session observation differs")


def comparable(name: str, text: str) -> str:
    """Exclude only the native pointer spelling in three declared boundary cases."""
    if name not in {
        "attempt-01-ordering-conversion-" + feed
        for feed in ("buffer", "unsigned", "utf8")
    }:
        return text
    lines = text.splitlines(keepends=True)
    addresses: list[bytes] = []
    for index, line in enumerate(lines):
        if not line.startswith(("return 9996 string ", "temp string ")):
            continue
        fields = line.split()
        size, encoded = fields[-2:]
        try:
            payload = bytes.fromhex(encoded)
            length = int(size)
        except ValueError as error:
            raise ArrayObservationMismatchError(
                "Invalid native pointer string"
            ) from error
        if (
            len(payload) != length
            or re.fullmatch(rb"\(array : 0x[0-9A-Fa-f]+\)", payload) is None
        ):
            raise ArrayObservationMismatchError("Invalid native pointer format/length")
        addresses.append(payload)
        lines[index] = " ".join(fields[:-2]) + " <native-array-address>\n"
    if len(addresses) != 2 or addresses[0] != addresses[1]:
        raise ArrayObservationMismatchError(
            "Native return/temp pointer identity differs"
        )
    return "".join(lines)


class ArrayObservationMismatchError(WorldCheckError):
    """The original session identity passed but its stdout differs from native."""
