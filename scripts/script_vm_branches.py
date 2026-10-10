# SPDX-License-Identifier: GPL-2.0-only
"""Actual private frame tests, targeted branch budgets and corruption witnesses."""

import shutil
from pathlib import Path

from scripts.gameplay_foundations import FoundationRun
from scripts.script_vm_iteration import ITERATION_BUDGETS, ITERATION_CONTROLS
from scripts.script_vm_observation import compare_observation, require_tests
from scripts.script_vm_provenance import digest, select_executable
from scripts.script_vm_switch import SWITCH_BUDGETS, SWITCH_CONTROLS
from scripts.world_check_support import (
    ROOT,
    Json,
    WorldCheckError,
    read_json,
    run,
    write_json,
)

FRAME_TESTS = (
    "vm::array_sessions::foreach::native_foreach_sessions",
    "vm::array_sessions::foreach::native_string_iterable_compiles_exactly_but_stops_at_typed_boundary",
    "vm::array_sessions::actual_native_owned_array_sessions",
    "vm::array_sessions::native_valid_object_boundaries_keep_exact_compilation",
    "runner::tests::program_clone_shares_one_literal_pool_and_failed_call_retains_real_program",
    "runner::tests::terminal_frame_releases_program_before_wrapper_drop",
    "runner::tests::compile_failure_keeps_previous_temporary_and_success_keeps_error_record",
    "vm::root_sessions::supported_root_sessions_match_native_frames_and_owners",
    "vm::root_sessions::compile_call_owner_and_root_replacement_match_native",
    "vm::root_sessions::native_success_outside_scalar_root_domain_is_explicitly_rejected",
    "realm::constants::tests::native_buffer_corpus_matches_compilation_and_published_table",
    "realm::constants::tests::temporary_updates_match_native_suspension_results",
    "realm::constants::tests::declaration_timing_preserves_old_or_new_binding_at_native_byte_boundaries",
    "realm::constants::tests::names_values_and_partial_enum_scratch_have_distinct_owners",
    "realm::constants::sessions::shared_session_effects_match_native_compile_lookup_and_execution",
    "realm::tests::failed_compilation_releases_literals_before_recompiling_in_same_realm",
    "realm::tests::realm_lifetimes_match_native_when_programs_and_runner_are_released",
    "realm::tests::independent_suspended_owners_match_native_when_one_runner_is_dropped",
    "realm::tests::intern_keys_are_released_when_the_last_value_drops",
    "realm::tests::parts_constructor_reinterns_foreign_strings_before_native_identity_equality",
    "realm::tests::failed_arithmetic_keeps_native_temporary_until_vm_drop",
    "vm::tests::strings_load_frame_matches_native_frames",
    "vm::tests::strings_update_frame_matches_native_frames",
    "vm::tests::strings_failure_frame_matches_native_frames",
    "vm::tests::scope_clear_matches_native_frames",
    "vm::tests::scope_noop_matches_native_frames",
    "vm::tests::local_alias_matches_native_frames",
    "vm::tests::expression_assignment_matches_native_frames",
    "vm::tests::iteration_update_frame_matches_native_frames",
    "vm::tests::iteration_loop_frame_matches_native_frames",
    "vm::tests::failed_update_matches_native_frames",
    "vm::tests::switch_case_frame_matches_native_frames",
    "vm::tests::switch_continue_frame_matches_native_frames",
)
FRAMES = (
    "strings_load_frame",
    "strings_update_frame",
    "strings_failure_frame",
    "scope_guard_clear",
    "scope_guard_noop",
    "local_alias",
    "expstate_target",
    "iteration_update_frame",
    "iteration_loop_frame",
    "iteration_failed_update",
    "switch_case_frame",
    "switch_continue_frame",
)
FRAME_CREDITS = (3, 2, 2, 100)
FRAME_SCHEDULES = {
    "strings_load_frame": (2,) * 25 + (100,),
    "strings_update_frame": (2,) * 25 + (100,),
    "strings_failure_frame": (2,) * 25 + (100,),
    "expstate_target": (3, 2, 2, 2, 2, 100),
    "iteration_update_frame": (2,) * 20 + (100,),
    "iteration_loop_frame": (2,) * 20 + (100,),
    "iteration_failed_update": (2, 2, 100),
    "switch_case_frame": (2,) * 20 + (100,),
    "switch_continue_frame": (2,) * 20 + (100,),
}
BUDGETS = (
    (
        ("while_sum", (0,), "suspend"),
        ("while_sum", (1,), "suspend"),
        ("while_sum", (37,), "suspend"),
        ("while_sum", (38,), "suspend"),
        ("while_sum", (39,), "return"),
        ("while_sum", (2,) * 45, "return"),
        ("local_alias", (0,), "suspend"),
        ("local_alias", (1,), "suspend"),
        ("local_alias", (4,), "suspend"),
        ("local_alias", (5,), "suspend"),
        ("local_alias", (6,), "return"),
        ("short_and", (0,), "suspend"),
        ("short_and", (1,), "suspend"),
        ("short_and", (2,) * 8, "return"),
        ("loop_continue", (2,) * 65, "return"),
        ("infinite", (2,) * 20, "suspend"),
    )
    + ITERATION_BUDGETS
    + SWITCH_BUDGETS
)

# Each target is actual retained native output, compared with a deliberately wrong record.
BRANCH_CONTROLS = {
    "assignment": ("expstate_target", "op 10 3 4 0 0", "op 10 1 4 0 0"),
    "jump": ("while_sum", "op 24 0 -8 0 0", "op 24 0 -7 0 0"),
    "polarity": ("if_true", "op 26 ", "op 25 "),
    "local": ("dmove", "op 23 3 1 4 2", "op 23 3 1 4 1"),
    "literal": ("literal_eq", "op 15 2 1 1 255", "op 15 2 1 1 0"),
    "scope-charge": ("while_sum", "return 9962 integer 6", "return 9963 integer 6"),
    "short-circuit": ("short_and", "return 9997 bool 0", "runtime_error 9995"),
}

BRANCH_CONTROLS.update(ITERATION_CONTROLS)
BRANCH_CONTROLS.update(SWITCH_CONTROLS)
BRANCH_CONTROLS.update(
    {
        "unicode-max-char": (
            "unicode_040.nut",
            "compile_error",
            "return 9998 integer 1",
        ),
        "unicode-nul-eof": ("unicode_075.nut", "integer 1", "integer 9"),
        "unicode-surrogate": (
            "unicode_020.nut",
            "return 9998 integer 1",
            "compile_error",
        ),
    }
)

LIB_BUILD = [
    "cargo",
    "test",
    "--locked",
    "-p",
    "ottd-script",
    "--lib",
    "--no-run",
    "--message-format=json",
]


def fixture_name(name: str) -> str:
    return (
        name
        if name.endswith(".nut")
        else (f"{name}.nut" if name.startswith("strings_") else f"branch_{name}.nut")
    )


def run_branches(builder: FoundationRun, rust: Path) -> Path:
    output = builder.output
    built = builder.run("build-libtests", LIB_BUILD)
    tests = select_executable(built.stdout, "ottd_script", "lib", True)
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
    for index, (name, credits, stage) in enumerate(BUDGETS):
        source = output / "inputs" / fixture_name(name)
        destination = output / "budgets" / str(index)
        native = run(
            [str(builder.oracle), str(source), *map(str, credits)],
            destination / "native",
        )
        actual = run([str(rust), str(source), *map(str, credits)], destination / "rust")
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


def validate_branches(directory: Path) -> None:
    identities = read_json(directory / "identities.json")
    match identities:
        case {"artifact_root": str() as root, "rust": {"path": str() as rust}}:
            origin = Path(root)
        case _:
            raise WorldCheckError("Missing branch observer identities")
    match read_json(directory / "branch-identities.json"):
        case {"path": str() as tests, "sha256": str() as sha}:
            pass
        case _:
            raise WorldCheckError("Missing private frame test identity")
    if (
        read_json(directory / "branch-identity-after.json")
        != {"path": tests, "sha256": sha}
        or digest(directory / "bin/lib-tests") != sha
    ):
        raise WorldCheckError("Private frame test executable changed")
    if (
        read_json(directory / "logs/build-libtests/argv.json") != LIB_BUILD
        or str(
            select_executable(
                (directory / "logs/build-libtests/stdout.log").read_text(),
                "ottd_script",
                "lib",
                True,
            )
        )
        != tests
    ):
        raise WorldCheckError("Private frame test build selection differs")
    if read_json(directory / "lib-tests/argv.json") != [
        "env",
        f"OTTD_SCRIPT_ROOT_CORPUS={origin / 'roots/inputs'}",
        f"OTTD_SCRIPT_ARRAY_CORPUS={origin / 'arrays/inputs'}",
        f"OTTD_SCRIPT_FOREACH_CORPUS={origin / 'foreach/inputs'}",
        tests,
        "--test-threads=1",
    ]:
        raise WorldCheckError("Private frame test selection differs")
    require_tests((directory / "lib-tests/stdout.log").read_text(), FRAME_TESTS)
    native = str(origin / "native/observe")
    for name in FRAMES:
        destination = directory / "frames" / name
        argv = [
            native,
            str(origin / "inputs" / fixture_name(name)),
            "--frames",
            *map(str, FRAME_SCHEDULES.get(name, FRAME_CREDITS)),
        ]
        if (
            read_json(destination / "argv.json") != argv
            or (destination / "stdout.log").read_text()
            != (ROOT / f"crates/ottd-script/tests/frames/{name}.txt").read_text()
        ):
            raise WorldCheckError("Native private frame witness differs")
    for index, (name, credits, stage) in enumerate(BUDGETS):
        destination = directory / "budgets" / str(index)
        for side, binary in (("native", native), ("rust", rust)):
            if read_json(destination / side / "argv.json") != [
                binary,
                str(origin / "inputs" / fixture_name(name)),
                *map(str, credits),
            ]:
                raise WorldCheckError("Branch budget selection differs")
        compare_observation(
            (destination / "native/stdout.log").read_text(),
            (destination / "rust/stdout.log").read_text(),
            stage,
        )
    for name, (fixture, old, new) in BRANCH_CONTROLS.items():
        relative = f"cases/{Path(fixture_name(fixture)).stem}/10000/native/stdout.log"
        base = directory / relative
        changed = directory / "controls" / f"branch-{name}" / "changed.stdout"
        expected_argv = [
            "diff",
            "-u",
            str(origin / relative),
            str(origin / changed.relative_to(directory)),
        ]
        if (
            old not in base.read_text()
            or changed.read_text() != base.read_text().replace(old, new)
            or read_json(changed.parent / "compare/argv.json") != expected_argv
            or not (changed.parent / "compare/stdout.log").read_text()
        ):
            raise WorldCheckError("Branch corruption was not compared")
