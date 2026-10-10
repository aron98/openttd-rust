# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 -m scripts.script_vm_evidence ARTIFACT_DIRECTORY
"""Exact scalar VM observations, execution and source admission."""

import sys
from pathlib import Path

from scripts.script_vm_observation import (
    compare_observation,
    require_tests,
)
from scripts.script_vm_policy import POLICY_STAGE, compare_case
from scripts.script_vm_provenance import (
    archive_files,
    digest,
    load_spec,
    select_executable,
    verify_archive,
    verify_sources,
)
from scripts.world_check_support import ROOT, WorldCheckError, at, read_json

__all__ = (
    "CONTROLS",
    "MANIFEST",
    "compare_observation",
    "package",
    "require_paths",
    "require_tests",
    "validate",
)

CONTROLS = {
    "value": ("integer 25", "integer 26"),
    "opcode": ("op 17 1 2 1 43", "op 17 1 2 1 45"),
    "ip": ("suspend 0 2", "suspend 0 3"),
    "debt": ("suspend -1 0", "suspend 0 0"),
}
MANIFEST = ROOT / "scripts/script-vm-manifest.json"


def require_paths(
    actual: set[str], expected: set[str], *, packaged: bool = True
) -> None:
    required = (
        expected | {"evidence-index.json", "evidence.tar.gz"} if packaged else expected
    )
    if actual != required:
        raise WorldCheckError("Incomplete or substituted VM evidence membership")


def validate(directory: Path, *, packaged: bool = True) -> None:
    spec = load_spec(MANIFEST)
    expected = set(spec.paths)
    actual = {
        str(p.relative_to(directory)) for p in directory.rglob("*") if p.is_file()
    }
    require_paths(actual, expected, packaged=packaged)
    empty_native = {
        "native/" + p
        for p, blob in spec.native
        if blob == "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391"
    }
    for path in directory.rglob("*"):
        if path.is_symlink():
            raise WorldCheckError("Symlink in VM evidence")
        if (
            path.is_file()
            and path.stat().st_size == 0
            and path.name not in ("stdout.log", "stderr.log")
            and str(path.relative_to(directory)) not in empty_native
        ):
            raise WorldCheckError("Empty substantive VM evidence")
    if (directory / "manifest.json").read_bytes() != MANIFEST.read_bytes():
        raise WorldCheckError("VM manifest identity changed")
    verify_sources(ROOT, directory / "native", spec)
    for name, sha in spec.sources:
        if digest(directory / "source" / name) != sha:
            raise WorldCheckError("Retained VM source changed")
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
    stages = dict(spec.stages)
    for path in directory.rglob("process.json"):
        relative = str(path.relative_to(directory))
        parts = path.relative_to(directory).parts
        if (
            len(parts) == 5
            and parts[0] == "cases"
            and parts[3] == "native"
            and stages.get("/".join(parts[1:3])) == POLICY_STAGE
        ):
            continue  # Exact policy receipt and source bytes are checked by compare_case below.
        status = int(relative.startswith("controls/"))
        expected_receipt = (
            {"returncode": 0}
            if relative.startswith("logs/")
            else {"returncode": status, "expected": status}
        )
        if read_json(path) != expected_receipt:
            raise WorldCheckError(f"VM process failed: {relative}")
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
    stages = dict(spec.stages)
    for name in spec.fixtures:
        source = ROOT / "scripts/compat/script-vm/fixtures" / name
        if digest(directory / "inputs" / name) != digest(source):
            raise WorldCheckError("VM input changed")
        for credit in spec.credits:
            key = f"{Path(name).stem}/{'-'.join(map(str, credit))}"
            case = directory / "cases" / key
            for side, binary in (
                ("native", str(Path(origin) / "native/observe")),
                ("rust", rust_path),
            ):
                argv = [binary, str(Path(origin) / "inputs" / name), *map(str, credit)]
                if read_json(case / side / "argv.json") != argv:
                    raise WorldCheckError("VM matrix invocation differs")
            compare_case(source.read_bytes(), case, stages[key])
    base = directory / "cases/precedence/0-1-2-3-100/native/stdout.log"
    for name, (old, new) in CONTROLS.items():
        changed = directory / "controls" / name / "changed.stdout"
        if (
            old not in base.read_text()
            or changed.read_text() != base.read_text().replace(old, new)
        ):
            raise WorldCheckError("VM control mutation differs")
        log = changed.parent / "compare"
        if not (log / "stdout.log").read_text() or read_json(log / "argv.json") != [
            "diff",
            "-u",
            str(Path(origin) / base.relative_to(directory)),
            str(Path(origin) / changed.relative_to(directory)),
        ]:
            raise WorldCheckError("VM corruption was not compared")
    from scripts.script_vm_branches import validate_branches

    validate_branches(directory)
    if read_json(directory / "summary.json") != {
        "cases": 7777,
        "fixtures": 850,
        "credits": 9,
        "tests": 55,
        "strict_comparisons": 7705,
        "undefined_input_rejections": 72,
        "controls": 25,
        "frame_tests": 9,
        "native_frames": 9,
        "branch_budget_cases": 127,
        "passed": True,
    }:
        raise WorldCheckError("VM summary differs")
    if (directory / "evidence-index.json").exists():
        index = read_json(directory / "evidence-index.json")
        if at(index, ("count",)) != len(spec.paths) or at(
            index, ("archive_sha256",)
        ) != digest(directory / "evidence.tar.gz"):
            raise WorldCheckError("VM archive identity differs")
        if at(index, ("files",)) != {p: digest(directory / p) for p in spec.paths}:
            raise WorldCheckError("VM raw indexed hash differs")
        verify_archive(directory, spec.paths)


def package(directory: Path) -> None:
    validate(directory, packaged=False)
    archive_files(directory, load_spec(MANIFEST).paths)


if __name__ == "__main__":
    validate(Path(sys.argv[1]).resolve())
    print("PASS scalar VM evidence")
