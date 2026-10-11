# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 -m unittest scripts.tests.test_script_vm_policy
"""Producer routing checks inherited by the existing policy test selectors."""
import os
import shutil
import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from scripts.script_vm_policy import POLICY_STAGE, UNDEFINED_INPUTS, observe_native
from scripts.world_check_support import ROOT, read_json


class PolicyProducerChecks(unittest.TestCase):
    def test_undefined_producer_never_invokes_native(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            marker = root / "native-invoked"
            argv = [
                sys.executable,
                "-c",
                "from pathlib import Path; import sys; Path(sys.argv[1]).touch()",
                str(marker),
            ]
            for index, source in enumerate(UNDEFINED_INPUTS):
                case = root / str(index)
                observe_native(argv, case, source)
                self.assertFalse(marker.exists())
                self.assertFalse((case / "native").exists())
                self.assertEqual(
                    read_json(case / "policy.json"),
                    {"native_executed": False, "reason": POLICY_STAGE},
                )

    def test_legacy_shell_never_invokes_native_for_policy_input(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            harness = root / "scripts/compat/script-vm"
            (harness / "fixtures").mkdir(parents=True)
            source, policy = next(iter(UNDEFINED_INPUTS.items()))
            _ = (harness / "fixtures/undefined.nut").write_bytes(source)
            shell = harness / "differential.sh"
            _ = shutil.copy2(ROOT / "scripts/compat/script-vm/differential.sh", shell)
            native = root / "native"
            marker = root / "native-invoked"
            _ = native.write_text(
                f"#!/bin/sh\ntouch '{marker}'\nprintf 'compile_error\\n'\n"
            )
            native.chmod(0o755)
            rust = root / "rust"
            _ = rust.write_text(
                "#!/bin/sh\nprintf 'compile_error\\n'\n"
                + f"printf '%s' '{policy.diagnostic()}' >&2\n"
            )
            rust.chmod(0o755)
            result = subprocess.run(
                ["/bin/sh", str(shell), str(native), str(rust), str(root / "out")],
                capture_output=True,
                text=True,
                check=False,
                env={**os.environ, "PYTHONPATH": str(ROOT)},
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(marker.exists())
            self.assertEqual(len(list((root / "out").rglob("policy.json"))), 9)

    def test_defined_unicode_strings_and_comments_invoke_native(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            marker = root / "native-invoked"
            argv = [
                sys.executable,
                "-c",
                "from pathlib import Path; import sys; Path(sys.argv[1]).touch()",
                str(marker),
            ]
            for index, source in enumerate(
                ('return "€";', "return/*€*/1;", "return//€\n1;")
            ):
                observe_native(argv, root / str(index), source.encode())
                self.assertTrue(marker.exists())
                self.assertTrue((root / str(index) / "native/process.json").is_file())
                self.assertFalse((root / str(index) / "policy.json").exists())
                marker.unlink()

