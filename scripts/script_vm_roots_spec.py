# SPDX-License-Identifier: GPL-2.0-only
"""Roots source specifications and corpus membership."""

from pathlib import Path
from typing import Final

from scripts.script_vm_provenance import digest
from scripts.world_check_support import ROOT, WorldCheckError, read_json

SOURCE: Final = Path("scripts/compat/script-vm/roots")


TESTS: Final = (
    "shared_root_and_independent_runner_state_survive_errors",
    "root_replacement_is_run_time_but_constants_are_compile_time",
    "supported_lookup_errors_do_not_conflate_native_unsupported_values",
    "host_preconditions_and_scalar_results_have_separate_ownership",
    "unsupported_root_source_remains_distinct_from_lexer_policy",
    "missing_out_of_scope_local_is_a_runtime_lookup",
    "checked_root_bytecode_preserves_index_and_receiver_errors",
)


BUILD: Final = [
    "cargo",
    "test",
    "--locked",
    "-p",
    "ottd-script",
    "--test",
    "root_runner",
    "--no-run",
    "--message-format=json",
]


def declarations() -> dict[str, dict[str, str]]:
    match read_json(ROOT / SOURCE / "witnesses.json"):
        case {"cases": list() as rows}:
            result: dict[str, dict[str, str]] = {}
            for row in rows:
                match row:
                    case {
                        "id": str() as name,
                        "session": str() as session,
                        "cwd": str() as cwd,
                        "capture": str() as capture,
                        "domain": str() as domain,
                    }:
                        if name in result:
                            raise WorldCheckError("Duplicate root session")
                        result[name] = {
                            "session": session,
                            "cwd": cwd,
                            "capture": capture,
                            "domain": domain,
                        }
                    case _:
                        raise WorldCheckError("Invalid root declaration")
        case _:
            raise WorldCheckError("Missing root declarations")
    for name, row in result.items():
        session = Path(row["session"])
        group = session.parts[0] if len(session.parts) == 3 else "."
        expected_id = (group + "-" if group != "." else "") + session.stem
        expected_capture = Path(group if group != "." else "native") / (
            session.stem + ".txt"
        )
        if (
            session.is_absolute()
            or ".." in session.parts
            or group not in {".", "ownership", "replacement"}
            or session.parent.name != "fixtures"
            or session.suffix != ".session"
            or row["cwd"] != group
            or name != expected_id
            or row["capture"] != str(expected_capture)
        ):
            raise WorldCheckError("Root session identity or relative path differs")
    sessions = {
        str(p.relative_to(ROOT / SOURCE)) for p in (ROOT / SOURCE).rglob("*.session")
    }
    if sessions != {row["session"] for row in result.values()}:
        raise WorldCheckError("Root declared session membership differs")
    domains = [row["domain"] for row in result.values()]
    if (
        len(result),
        domains.count("strict-stateful-projection"),
        domains.count("explicit-syntax-boundary"),
        domains.count("explicit-runtime-boundary"),
    ) != (90, 75, 9, 6):
        raise WorldCheckError("Incomplete root session domains")
    return result


def cases() -> tuple[str, ...]:
    return tuple(declarations())


def input_names() -> tuple[str, ...]:
    return tuple(
        sorted(
            str(p.relative_to(ROOT / SOURCE))
            for p in (ROOT / SOURCE).rglob("*")
            if p.is_file() and "fixtures" in p.relative_to(ROOT / SOURCE).parts
        )
    )


def argv(origin: Path, name: str) -> list[str]:
    row = declarations()[name]
    return [
        str(origin / "native/observe"),
        "--root-slots-session",
        str(Path(row["session"]).relative_to(row["cwd"])),
    ]


def capture(name: str) -> Path:
    return Path(declarations()[name]["capture"])


def working_directory(origin: Path, name: str) -> Path:
    return origin / "roots/inputs" / declarations()[name]["cwd"]


def validate_corpus() -> None:
    """Bind every native capture/input to the same production Rust session corpus."""
    match read_json(ROOT / SOURCE / "witnesses.json"):
        case {"files": dict() as files}:
            pass
        case _:
            raise WorldCheckError("Missing root corpus file map")
    rust = ROOT / "crates/ottd-script/tests/roots"
    actual = {
        str(p.relative_to(ROOT / SOURCE))
        for p in (ROOT / SOURCE).rglob("*")
        if p.is_file() and p.name != "witnesses.json"
    }
    if actual != set(files) or len(actual) != 212:
        raise WorldCheckError("Root corpus membership differs")
    for name, sha in files.items():
        if (
            not isinstance(sha, str)
            or digest(ROOT / SOURCE / name) != sha
            or digest(rust / name) != sha
        ):
            raise WorldCheckError("Root source or native capture differs")
    _ = declarations()
