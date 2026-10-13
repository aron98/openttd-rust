# SPDX-License-Identifier: GPL-2.0-only
"""Fresh native string ownership, byte-feed and raw-f32 formatting witnesses."""

import shutil
from pathlib import Path

from scripts.script_vm_provenance import digest
from scripts.world_check_support import ROOT, WorldCheckError, read_json, run

SESSION_ARGS = {
    "compile-failure": (
        "--compile-failure",
        "strings_compile_failure.nut",
        "strings_realm_literal.nut",
    ),
    "realm": (
        "--realm",
        "strings_realm_literal.nut",
        "strings_realm_literal.nut",
        "strings_realm_dynamic.nut",
        "strings_realm_error.nut",
    ),
    "parallel": ("--parallel", "strings_realm_literal.nut"),
    "terminal": ("--terminal", "strings_failure_frame.nut"),
}
FLOAT_BITS = "scripts/compat/script-vm/float-bits.txt"
FEED_FILES = tuple(f"scripts/compat/script-vm/feeds/{index}.nut" for index in range(9))
STRING_CONTROLS = {
    "session-action": ("sessions/realm", "refs drop_first 1\n", ""),
    "byte-length": ("sessions/parallel", "string 11 ", "string 10 "),
    "lifetime": ("sessions/parallel", "refs loaded 3", "refs loaded 2"),
    "failure-stage": ("sessions/compile-failure", "compile_error", "compiled"),
}


def session_argv(origin: Path, name: str) -> list[str]:
    mode, *fixtures = SESSION_ARGS[name]
    return [
        str(origin / "native/observe"),
        mode,
        *(str(origin / "inputs" / fixture) for fixture in fixtures),
    ]


def expected(relative: str) -> str:
    name = relative.replace("/", "-")
    observed = (ROOT / f"scripts/compat/script-vm/strings/{name}.txt").read_text()
    if relative.startswith("sessions/"):
        session = relative.removeprefix("sessions/").replace("-", "_")
        rust = ROOT / f"crates/ottd-script/tests/frames/strings_{session}.txt"
        if observed != rust.read_text():
            raise WorldCheckError("Native session and Rust lifetime golden differ")
    return observed


def check_format(native: str, rust: str, bits: str) -> None:
    requested = bits.splitlines()
    if (
        len(requested) != 23071
        or len(set(requested)) != 23071
        or native != rust
        or [line.split(" ", 1)[0] for line in native.splitlines()] != requested
    ):
        raise WorldCheckError("Native string float-format witness differs")


def run_strings(output: Path, rust: Path) -> None:
    destination = output / "strings"
    destination.mkdir()
    for source, name in (
        (FLOAT_BITS, "float-bits.txt"),
        *((path, f"feed-{index}.nut") for index, path in enumerate(FEED_FILES)),
    ):
        _ = shutil.copy2(ROOT / source, destination / name)
    for name in SESSION_ARGS:
        relative = f"sessions/{name}"
        observed = run(session_argv(output, name), destination / relative)
        if observed.stdout != expected(relative):
            raise WorldCheckError("Native string session differs")
    for index, _ in enumerate(FEED_FILES):
        for mode in ("unsigned", "utf8"):
            relative = f"feeds/{index}-{mode}"
            observed = run(
                [
                    str(output / "native/observe"),
                    "--feed",
                    mode,
                    str(destination / f"feed-{index}.nut"),
                ],
                destination / relative,
            )
            if observed.stdout != expected(relative):
                raise WorldCheckError("Native source-feed witness differs")
    for side, binary in (("native", output / "native/observe"), ("rust", rust)):
        _ = run(
            [str(binary), "--format", str(destination / "float-bits.txt")],
            destination / "format" / side,
        )
    check_format(
        (destination / "format/native/stdout.log").read_text(),
        (destination / "format/rust/stdout.log").read_text(),
        (destination / "float-bits.txt").read_text(),
    )
    for name, (relative, old, new) in STRING_CONTROLS.items():
        source = destination / relative / "stdout.log"
        changed = output / "controls" / f"string-{name}" / "changed.stdout"
        changed.parent.mkdir(parents=True)
        if old not in source.read_text():
            raise WorldCheckError("String control target missing")
        _ = changed.write_text(source.read_text().replace(old, new))
        _ = run(
            ["diff", "-u", str(source), str(changed)], changed.parent / "compare", 1
        )


def validate_strings(directory: Path, origin: Path, rust: str) -> None:
    destination = directory / "strings"
    for source, name in (
        (FLOAT_BITS, "float-bits.txt"),
        *((path, f"feed-{index}.nut") for index, path in enumerate(FEED_FILES)),
    ):
        if digest(ROOT / source) != digest(destination / name):
            raise WorldCheckError("String witness input changed")
    for name in SESSION_ARGS:
        relative = f"sessions/{name}"
        if read_json(destination / relative / "argv.json") != session_argv(
            origin, name
        ) or (destination / relative / "stdout.log").read_text() != expected(relative):
            raise WorldCheckError("Native string session differs")
    for index, _ in enumerate(FEED_FILES):
        for mode in ("unsigned", "utf8"):
            relative = f"feeds/{index}-{mode}"
            argv = [
                str(origin / "native/observe"),
                "--feed",
                mode,
                str(origin / "strings" / f"feed-{index}.nut"),
            ]
            if read_json(destination / relative / "argv.json") != argv or (
                destination / relative / "stdout.log"
            ).read_text() != expected(relative):
                raise WorldCheckError("Native source-feed witness differs")
    for side, binary in (("native", str(origin / "native/observe")), ("rust", rust)):
        argv = [binary, "--format", str(origin / "strings/float-bits.txt")]
        if read_json(destination / "format" / side / "argv.json") != argv:
            raise WorldCheckError("String format invocation differs")
    check_format(
        (destination / "format/native/stdout.log").read_text(),
        (destination / "format/rust/stdout.log").read_text(),
        (destination / "float-bits.txt").read_text(),
    )
    validate_controls(directory, origin)


def validate_controls(directory: Path, origin: Path) -> None:
    destination = directory / "strings"
    for name, (relative, old, new) in STRING_CONTROLS.items():
        source = destination / relative / "stdout.log"
        changed = directory / "controls" / f"string-{name}" / "changed.stdout"
        argv = [
            "diff",
            "-u",
            str(origin / "strings" / relative / "stdout.log"),
            str(origin / changed.relative_to(directory)),
        ]
        if (
            old not in source.read_text()
            or changed.read_text() != source.read_text().replace(old, new)
            or read_json(changed.parent / "compare/argv.json") != argv
            or not (changed.parent / "compare/stdout.log").read_text()
        ):
            raise WorldCheckError("String corruption was not compared")
