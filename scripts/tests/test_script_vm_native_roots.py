"""Corrupt real finite native root receipts at physical and ownership boundaries."""

import shutil
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from scripts.script_vm_inputs import OBSERVER_INPUTS
from scripts.script_vm_native_roots import parse
from scripts.script_vm_native_roots_audit import recorder_pair, scenario
from scripts.script_vm_provenance import verify_observer_sources
from scripts.tests.script_vm_policy_assertions import world_check_rejected

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "scripts/compat/script-vm/tables/fixtures/native-roots"


class NativeRootReceiptTests(unittest.TestCase):
    def test_actual_idle_recorder_pair_and_physical_roots(self) -> None:
        off, on = (
            (FIXTURES / "idle-off.txt").read_bytes(),
            (FIXTURES / "idle-on.txt").read_bytes(),
        )
        trace = recorder_pair("idle", off, on)
        assert len(trace.snapshots) == 2
        assert all(len(item.stack) == 256 for item in trace.snapshots)
        assert {label for label, _ in trace.free_events} == {"V", "O"}

    def test_allocated_slot_order_and_complete_chain_are_mandatory(self) -> None:
        raw = (FIXTURES / "idle-on.txt").read_bytes()
        for before, after in (
            (b"roots stack 0\n", b""),
            (b"roots stack 1\n", b"roots stack 0\n"),
            (b"roots chain 85 ", b"roots chain 84 "),
            (b"roots object error ", b"roots object temporary "),
            (b"roots object slot 16777217", b"roots object slot 999"),
            (b"recorder 1\n", b"recorder 0\n"),
        ):
            assert before in raw
            with self.subTest(before=before), world_check_rejected():
                _ = parse(raw.replace(before, after, 1))
        _ = parse(raw)

    def test_syntactically_valid_extra_root_owner_is_rejected(self) -> None:
        raw = (FIXTURES / "idle-on.txt").read_bytes()
        before = b"roots object root 167772192 O 1 "
        assert before in raw
        changed = raw.replace(before, b"roots object root 167772192 O 2 ", 1)
        _ = parse(changed)
        with world_check_rejected():
            _ = scenario("idle", changed)
        _ = scenario("idle", raw)

    def test_actual_string_bytes_and_retirement_identity_are_strict(self) -> None:
        raw = (FIXTURES / "persistent-on.txt").read_bytes()
        string = next(
            line for line in raw.splitlines() if line.startswith(b"roots bytes ")
        )
        fields = string.split(b" ")
        fields[3] = b"0"
        free = next(line for line in raw.splitlines() if line.startswith(b"free "))
        for changed in (
            raw.replace(string, b" ".join(fields), 1),
            raw.replace(free + b"\n", free + b"\n" + free + b"\n", 1),
        ):
            with world_check_rejected():
                _ = parse(changed)
        _ = scenario("persistent", raw)

    def test_friend_local_stops_at_the_source_safe_scalar_boundary(self) -> None:
        raw = (FIXTURES / "friend-local-on.txt").read_bytes()
        _ = scenario("friend-local", raw)
        with world_check_rejected():
            _ = scenario("friend-local", raw.replace(b"roots detached-head 1 -\n", b""))

    def test_every_new_compiled_header_requires_bytes_mode_and_regular_copy(
        self,
    ) -> None:
        for helper in (
            "native_roots_trace.hpp",
            "native_roots_sessions.hpp",
            "native_roots_refs.hpp",
        ):
            for change in ("missing", "bytes", "mode", "symlink"):
                with (
                    self.subTest(helper=helper, change=change),
                    TemporaryDirectory() as directory,
                ):
                    native = Path(directory)
                    for name in OBSERVER_INPUTS:
                        _ = shutil.copy2(
                            ROOT / "scripts/compat/script-vm" / name, native / name
                        )
                    verify_observer_sources(ROOT, native)
                    target = native / helper
                    match change:
                        case "bytes":
                            _ = target.write_bytes(target.read_bytes() + b"\n")
                        case "mode":
                            target.chmod(0o600)
                        case "missing" | "symlink":
                            target.unlink()
                            if change == "symlink":
                                target.symlink_to(
                                    ROOT / "scripts/compat/script-vm" / helper
                                )
                    with world_check_rejected():
                        verify_observer_sources(ROOT, native)
                    target.unlink(missing_ok=True)
                    _ = shutil.copy2(ROOT / "scripts/compat/script-vm" / helper, target)
                    verify_observer_sources(ROOT, native)
