# SPDX-License-Identifier: GPL-2.0-only
"""Untrusted process receipt types cannot masquerade as successful native runs."""

from __future__ import annotations

from pathlib import Path

import pytest

from scripts.gameplay_foundations import digest
from scripts.house_runtime_evidence import native_receipts
from scripts.world_check_support import Json, WorldCheckError, write_json


@pytest.fixture
def native_capture(tmp_path: Path) -> tuple[Path, Path]:
    """Construct only receipt/hash inputs; this fixture executes no game engine."""
    binary = tmp_path / "observer"
    source = tmp_path / "input.sav"
    _ = binary.write_bytes(b"unit binary identity")
    _ = source.write_bytes(b"unit saved input identity")
    capture = tmp_path / "capture"
    (capture / "logs/native").mkdir(parents=True)
    write_json(capture / "protocol.json", {"actions": []})
    _ = (capture / "openttd.cfg").write_text("[gui]\nautosave = off\n")
    write_json(capture / "results.json", {"actions": []})
    write_json(
        capture / "bindings.json",
        {
            "binary_sha256": digest(binary),
            "protocol_sha256": digest(capture / "protocol.json"),
            "config_sha256": digest(capture / "openttd.cfg"),
            "input": str(source),
            "input_sha256": digest(source),
        },
    )
    write_json(capture / "logs/native/process.json", {"returncode": 0, "expected": 0})
    return capture, binary


def test_integer_success_receipt_is_admitted(native_capture: tuple[Path, Path]) -> None:
    # Given matching inputs and canonical successful process metadata.
    capture, binary = native_capture
    # When reading the receipt, then its integer statuses are accepted.
    native_receipts(capture, binary)


@pytest.mark.parametrize("field", ["returncode", "expected"])
@pytest.mark.parametrize("replacement", [False, 0.0, None])
def test_noninteger_status_is_rejected(
    native_capture: tuple[Path, Path], field: str, replacement: Json
) -> None:
    # Given valid identity hashes and an untrusted process-status type.
    capture, binary = native_capture
    process: dict[str, Json] = {"returncode": 0, "expected": 0}
    process[field] = replacement
    write_json(capture / "logs/native/process.json", process)
    # When validating that receipt, then it cannot certify a successful process.
    with pytest.raises(WorldCheckError):
        native_receipts(capture, binary)
