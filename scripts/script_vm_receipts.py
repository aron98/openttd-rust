# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through scripts/check-script-vm.py.
"""Build selection, executable identities and process/test receipt admission."""
from pathlib import Path

from scripts.script_vm_arrays import CONTROLS as ARRAY_CONTROLS
from scripts.script_vm_arrays import declarations as array_declarations
from scripts.script_vm_constants import CONTROLS as CONSTANT_CONTROLS
from scripts.script_vm_foreach import CONTROLS as FOREACH_CONTROLS
from scripts.script_vm_observation import require_tests
from scripts.script_vm_provenance import Spec, digest, select_executable
from scripts.script_vm_roots import CONTROLS as ROOT_CONTROLS
from scripts.world_check_support import ROOT, WorldCheckError, read_json


def validate_identities(directory: Path) -> tuple[str, str, str]:
    identities = read_json(directory / "identities.json")
    match identities:
        case {
            "artifact_root": str() as origin,
            "rust": {"path": str() as rust_path, "sha256": str() as rust_sha},
            "tests": {"path": str() as test_path, "sha256": str() as test_sha},
            "native": str() as native_sha,
        }:
            pass
        case _:
            raise WorldCheckError("Missing VM executable identities")
    for relative, sha in (
        ("bin/rust-observe", rust_sha),
        ("bin/scalar-tests", test_sha),
        ("native/observe", native_sha),
    ):
        if digest(directory / relative) != sha:
            raise WorldCheckError("VM executable changed")
    if read_json(directory / "identity-after.json") != identities:
        raise WorldCheckError("VM identity changed during execution")
    return origin, rust_path, test_path


def validate_builds(directory: Path, identities: tuple[str, str, str]) -> None:
    origin, rust_path, test_path = identities
    expected_builds = {
        "build-rust": [
            "cargo",
            "build",
            "--locked",
            "-p",
            "ottd-script",
            "--example",
            "observe",
            "--message-format=json",
        ],
        "build-tests": [
            "cargo",
            "test",
            "--locked",
            "-p",
            "ottd-script",
            "--test",
            "scalar",
            "--no-run",
            "--message-format=json",
        ],
        "native-compiler": ["c++", "--version"],
        "rust-compiler": ["rustc", "--version"],
    }
    for label, argv in expected_builds.items():
        if read_json(directory / "logs" / label / "argv.json") != argv:
            raise WorldCheckError("VM build selection differs")
    match read_json(directory / "logs/native-build/argv.json"):
        case ["sh", str() as builder, str(), str() as destination] if builder == str(
            ROOT / "scripts/compat/script-vm/build-native.sh"
        ) and destination == str(Path(origin) / "native"):
            pass
        case _:
            raise WorldCheckError("VM native builder differs")
    for name, target, kind, testing, path in (
        ("build-rust", "observe", "example", False, rust_path),
        ("build-tests", "scalar", "test", True, test_path),
    ):
        selected = select_executable(
            (directory / "logs" / name / "stdout.log").read_text(),
            target,
            kind,
            testing,
        )
        if str(selected) != path:
            raise WorldCheckError("VM Cargo identity differs")


def validate_processes(directory: Path) -> None:
    for path in directory.rglob("process.json"):
        relative = str(path.relative_to(directory))
        parts = path.relative_to(directory).parts
        status = int(relative.startswith("controls/"))
        if (
            len(parts) > 2
            and parts[0] == "controls"
            and parts[1]
            in (
                {f"constant-{name}" for name in CONSTANT_CONTROLS}
                | {f"root-{name}" for name in ROOT_CONTROLS}
                | {f"array-{name}" for name in ARRAY_CONTROLS}
                | {f"foreach-{name}" for name in FOREACH_CONTROLS}
            )
        ):
            status = int(parts[2] == "admit-mutant")
        if len(parts) == 4 and parts[:2] == ("arrays", "sessions"):
            status = array_declarations()[parts[2]].status
        expected_receipt = (
            {"returncode": 0}
            if relative.startswith("logs/")
            else {"returncode": status, "expected": status}
        )
        if read_json(path) != expected_receipt:
            raise WorldCheckError(f"VM process failed: {relative}")


def validate_tests(directory: Path, spec: Spec, test_path: str) -> None:
    if read_json(directory / "tests/argv.json") != [test_path, "--test-threads=1"]:
        raise WorldCheckError("VM test selection changed")
    require_tests((directory / "tests/stdout.log").read_text(), spec.tests)
    zero = (directory / "zero-tests/stdout.log").read_text()
    if "test result: ok. 0 passed; 0 failed; 0 ignored;" not in zero:
        raise WorldCheckError("Missing actual zero-test negative")
    try:
        require_tests(zero, spec.tests)
    except WorldCheckError:
        pass
    else:
        raise WorldCheckError("Zero-test evidence admitted")


def validate_receipts(directory: Path, spec: Spec) -> tuple[str, str]:
    identities = validate_identities(directory)
    origin, rust_path, test_path = identities
    validate_builds(directory, identities)
    validate_processes(directory)
    validate_tests(directory, spec, test_path)
    return origin, rust_path
