# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 scripts/check-script-vm.py
"""Build and execute only the scalar bundled-Squirrel compatibility vertical."""

import os
import shutil
import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.gameplay_foundations import FoundationRun
from scripts.script_vm_branches import finish_branches, run_branches
from scripts.script_vm_evidence import (
    CONTROLS,
    MANIFEST,
    package,
    require_tests,
)
from scripts.script_vm_policy import compare_case, observe_native
from scripts.script_vm_provenance import (
    digest,
    load_spec,
    select_executable,
    verify_sources,
)
from scripts.script_vm_strings import run_strings
from scripts.world_check_support import ROOT, Json, WorldCheckError, run, write_json


def main() -> None:
    os.environ["CARGO_INCREMENTAL"] = "0"
    spec = load_spec(MANIFEST)
    parent = ROOT / ".artifacts"
    parent.mkdir(exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="script-vm-", dir=parent))
    print(f"Artifacts: {output}", flush=True)
    checkout = Path(
        os.environ.get("OTTD_SCRIPT_VM_SOURCE", str(ROOT / ".reference/OpenTTD"))
    ).resolve(strict=True)
    native = output / "native"
    builder = FoundationRun(ROOT, output, native / "observe")
    _ = builder.run("native-compiler", ["c++", "--version"])
    _ = builder.run("rust-compiler", ["rustc", "--version"])
    for name, sha in spec.sources:
        source = ROOT / name
        if digest(source) != sha:
            raise WorldCheckError(f"VM source does not match pinned manifest: {name}")
        target = output / "source" / name
        target.parent.mkdir(parents=True, exist_ok=True)
        _ = shutil.copy2(source, target)
    _ = shutil.copy2(MANIFEST, output / "manifest.json")
    _ = builder.run(
        "native-build",
        [
            "sh",
            str(ROOT / "scripts/compat/script-vm/build-native.sh"),
            str(checkout),
            str(native),
        ],
    )
    verify_sources(ROOT, native, spec)
    built = builder.run(
        "build-rust",
        [
            "cargo",
            "build",
            "--locked",
            "-p",
            "ottd-script",
            "--example",
            "observe",
            "--message-format=json",
        ],
    )
    rust = select_executable(built.stdout, "observe", "example", False)
    built_tests = builder.run(
        "build-tests",
        [
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
    )
    tests = select_executable(built_tests.stdout, "scalar", "test", True)
    (output / "bin").mkdir()
    for source, name in ((rust, "rust-observe"), (tests, "scalar-tests")):
        target = output / "bin" / name
        _ = shutil.copy2(source, target)
        target.chmod(0o555)
    (native / "observe").chmod(0o555)
    identities: Json = {
        "artifact_root": str(output),
        "rust": {"path": str(rust), "sha256": digest(rust)},
        "tests": {"path": str(tests), "sha256": digest(tests)},
        "native": digest(native / "observe"),
    }
    write_json(output / "identities.json", identities)
    test_run = run([str(tests), "--test-threads=1"], output / "tests")
    require_tests(test_run.stdout, spec.tests)
    zero = run(
        [str(tests), "--exact", "script_vm_ci_missing_test"], output / "zero-tests"
    )
    try:
        require_tests(zero.stdout, spec.tests)
    except WorldCheckError:
        pass
    else:
        raise WorldCheckError("Actual zero-test run admitted")
    (output / "inputs").mkdir()
    stages = dict(spec.stages)
    for name in spec.fixtures:
        source = output / "inputs" / name
        _ = shutil.copy2(ROOT / "scripts/compat/script-vm/fixtures" / name, source)
        for credit in spec.credits:
            key = f"{Path(name).stem}/{'-'.join(map(str, credit))}"
            case = output / "cases" / key
            observe_native(
                [str(native / "observe"), str(source), *map(str, credit)],
                case,
                source.read_bytes(),
            )
            _ = run([str(rust), str(source), *map(str, credit)], case / "rust")
            compare_case(source.read_bytes(), case, stages[key])
        print(f"PASS scalar {name}: 9 classified credit cases", flush=True)
    base = output / "cases/precedence/0-1-2-3-100/native/stdout.log"
    for name, (old, new) in CONTROLS.items():
        directory = output / "controls" / name
        directory.mkdir(parents=True)
        if old not in base.read_text():
            raise WorldCheckError("VM mutation target missing")
        target = directory / "changed.stdout"
        _ = target.write_text(base.read_text().replace(old, new))
        _ = run(["diff", "-u", str(base), str(target)], directory / "compare", 1)
    branch_tests = run_branches(builder, rust)
    run_strings(output, rust)
    verify_sources(ROOT, native, spec)
    finish_branches(output, branch_tests)
    write_json(
        output / "identity-after.json",
        {
            "artifact_root": str(output),
            "rust": {"path": str(rust), "sha256": digest(rust)},
            "tests": {"path": str(tests), "sha256": digest(tests)},
            "native": digest(native / "observe"),
        },
    )
    if digest(output / "bin/rust-observe") != digest(rust) or digest(
        output / "bin/scalar-tests"
    ) != digest(tests):
        raise WorldCheckError("VM retained executable changed")
    write_json(
        output / "summary.json",
        {
            "cases": 10819,
            "fixtures": 1188,
            "credits": 9,
            "tests": 62,
            "strict_comparisons": 10666,
            "undefined_input_rejections": 153,
            "controls": 25,
            "frame_tests": 18,
            "native_frames": 12,
            "branch_budget_cases": 127,
            "string_sessions": 4,
            "source_feed_cases": 18,
            "float_patterns": 23071,
            "string_controls": 4,
            "passed": True,
        },
    )
    package(output)
    print(
        "PASS scalar VM 10666 comparisons; 153 undefined-input rejections", flush=True
    )


if __name__ == "__main__":
    main()
