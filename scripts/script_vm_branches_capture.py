# SPDX-License-Identifier: GPL-2.0-only
"""Branches capture production and executable binding."""

import shutil
from pathlib import Path

from scripts.gameplay_foundations import FoundationRun
from scripts.script_vm_branches_spec import (
    BRANCH_CONTROLS,
    BUDGETS,
    FRAME_CREDITS,
    FRAME_SCHEDULES,
    FRAME_TESTS,
    FRAMES,
    LIB_BUILD,
    fixture_name,
)
from scripts.script_vm_observation import compare_observation, require_tests
from scripts.script_vm_provenance import digest, select_executable
from scripts.world_check_support import (
    ROOT,
    Json,
    WorldCheckError,
    read_json,
    run,
    write_json,
)


def run_branches(builder: FoundationRun, rust: Path) -> Path:
    output = builder.output
    built = builder.run("build-libtests", LIB_BUILD)
    tests = select_executable(built.stdout, "ottd_script", "lib", test=True)
    target = output / "bin/lib-tests"
    _ = shutil.copy2(tests, target)
    target.chmod(0o555)
    write_json(
        output / "branch-identities.json", {"path": str(tests), "sha256": digest(tests)}
    )
    actual = run(
        [
            "env",
            f"OTTD_SCRIPT_ROOT_CORPUS={output / 'roots/inputs'}",
            f"OTTD_SCRIPT_ARRAY_CORPUS={output / 'arrays/inputs'}",
            f"OTTD_SCRIPT_FOREACH_CORPUS={output / 'foreach/inputs'}",
            str(tests),
            "--test-threads=1",
            "--exact",
            *FRAME_TESTS,
        ],
        output / "lib-tests",
    )
    require_tests(actual.stdout, FRAME_TESTS)
    for name in FRAMES:
        source = output / "inputs" / fixture_name(name)
        observed = run(
            [
                str(builder.oracle),
                str(source),
                "--frames",
                *map(str, FRAME_SCHEDULES.get(name, FRAME_CREDITS)),
            ],
            output / "frames" / name,
        )
        if (
            observed.stdout
            != (ROOT / f"crates/ottd-script/tests/frames/{name}.txt").read_text()
        ):
            raise WorldCheckError("Native private frame witness differs")
    for index, (name, budget, stage) in enumerate(BUDGETS):
        source = output / "inputs" / fixture_name(name)
        destination = output / "budgets" / str(index)
        native = run(
            [str(builder.oracle), str(source), *map(str, budget)],
            destination / "native",
        )
        actual = run([str(rust), str(source), *map(str, budget)], destination / "rust")
        compare_observation(native.stdout, actual.stdout, stage)
    for name, (fixture, old, new) in BRANCH_CONTROLS.items():
        base = (
            output / f"cases/{Path(fixture_name(fixture)).stem}/10000/native/stdout.log"
        )
        directory = output / "controls" / f"branch-{name}"
        directory.mkdir(parents=True)
        if old not in base.read_text():
            raise WorldCheckError("Branch mutation target missing")
        changed = directory / "changed.stdout"
        _ = changed.write_text(base.read_text().replace(old, new))
        _ = run(["diff", "-u", str(base), str(changed)], directory / "compare", 1)
    return tests


def finish_branches(output: Path, tests: Path) -> None:
    identity: Json = {"path": str(tests), "sha256": digest(tests)}
    if read_json(output / "branch-identities.json") != identity or digest(
        output / "bin/lib-tests"
    ) != digest(tests):
        raise WorldCheckError("Private frame test executable changed")
    write_json(output / "branch-identity-after.json", identity)
