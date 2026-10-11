# SPDX-License-Identifier: GPL-2.0-only
"""Arrays source specifications and corpus membership."""

from dataclasses import dataclass
from pathlib import Path
from typing import Final

from scripts.script_vm_provenance import digest
from scripts.world_check_support import ROOT, WorldCheckError, read_json

SOURCE: Final = Path("scripts/compat/script-vm/arrays")


TESTS: Final = (
    "scalar_storage_and_alias_identity",
    "host_insertions_retain_container_identity_and_check_missing_indices_first",
    "root_affinity_is_checked_without_cloning_or_replacing_foreign_arrays",
    "returned_array_and_string_outlive_all_context_wrappers",
    "one_program_allocates_fresh_arrays_on_each_execution",
    "arrays_are_not_mutable_compiler_literals",
    "actual_native_scalar_idioms_and_defined_float_indices",
    "nested_arrays_and_cycles_are_executable_graph_consumers",
    "unsupported_runtime_domains_and_actual_missing_indices_stay_distinct",
    "float_index_conversion_covers_defined_i64_edge_and_nonfinite_bits",
    "malformed_collection_bytecode_fails_without_unchecked_access",
)


BUILD: Final = [
    "cargo",
    "test",
    "--locked",
    "-p",
    "ottd-script",
    "--test",
    "arrays",
    "--no-run",
    "--message-format=json",
]


@dataclass(frozen=True, slots=True)
class Session:
    session: str
    cwd: str
    capture: str
    domain: str
    status: int
    stderr: str


def declarations() -> dict[str, Session]:
    match read_json(ROOT / SOURCE / "witnesses.json"):
        case {"cases": list() as rows}:
            result: dict[str, Session] = {}
            for row in rows:
                match row:
                    case {
                        "id": str() as name,
                        "session": str() as session,
                        "cwd": str() as cwd,
                        "capture": str() as capture,
                        "domain": str() as domain,
                        "status": int() as status,
                        "stderr": str() as stderr,
                    }:
                        if name in result:
                            raise WorldCheckError("Duplicate array session")
                        result[name] = Session(
                            session, cwd, capture, domain, status, stderr
                        )
                    case _:
                        raise WorldCheckError("Invalid array declaration")
        case _:
            raise WorldCheckError("Missing array declarations")
    for name, row in result.items():
        session = Path(row.session)
        if (
            session.is_absolute()
            or ".." in session.parts
            or len(session.parts) != 3
            or row.cwd not in {"attempt-01", "attempt-02", "attempt-03"}
            or session.parent != Path(row.cwd) / "fixtures"
            or session.suffix != ".session"
            or name != row.cwd + "-" + session.stem
            or row.capture != str(Path(row.cwd) / "native" / (session.stem + ".txt"))
            or row.status != (65 if row.domain == "protocol-refusal" else 0)
            or bool(row.stderr) != (row.domain == "protocol-refusal")
        ):
            raise WorldCheckError("Array session identity or relative path differs")
    sessions = {
        str(p.relative_to(ROOT / SOURCE)) for p in (ROOT / SOURCE).rglob("*.session")
    }
    if sessions != {row.session for row in result.values()}:
        raise WorldCheckError("Array declared session membership differs")
    domains = [row.domain for row in result.values()]
    if (
        len(result),
        domains.count("strict-stateful-projection"),
        domains.count("explicit-runtime-boundary"),
        domains.count("protocol-refusal"),
    ) != (93, 78, 12, 3):
        raise WorldCheckError("Incomplete array session domains")
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
        "--array-session",
        str(Path(row.session).relative_to(row.cwd)),
    ]


def capture(name: str) -> Path:
    return Path(declarations()[name].capture)


def working_directory(origin: Path, name: str) -> Path:
    return origin / "arrays/inputs" / declarations()[name].cwd


def validate_corpus() -> None:
    """Bind every native capture/input to the same production Rust session corpus."""
    match read_json(ROOT / SOURCE / "witnesses.json"):
        case {"files": dict() as files}:
            pass
        case _:
            raise WorldCheckError("Missing array corpus file map")
    rust = ROOT / "crates/ottd-script/tests/arrays-native"
    actual = {
        str(p.relative_to(ROOT / SOURCE))
        for p in (ROOT / SOURCE).rglob("*")
        if p.is_file() and p.name != "witnesses.json"
    }
    if actual != set(files) or len(actual) != 228:
        raise WorldCheckError("Array corpus membership differs")
    for name, sha in files.items():
        if (
            not isinstance(sha, str)
            or digest(ROOT / SOURCE / name) != sha
            or digest(rust / name) != sha
        ):
            raise WorldCheckError("Array source or native capture differs")
    _ = declarations()
