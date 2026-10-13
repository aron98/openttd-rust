# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: python3 -m scripts.script_vm_evidence ARTIFACT_DIRECTORY
"""Exact scalar VM observations, execution and source admission."""

import sys
from pathlib import Path

from scripts.script_vm_arrays import validate_arrays
from scripts.script_vm_branches import validate_branches
from scripts.script_vm_constants import validate_constants
from scripts.script_vm_foreach import validate_foreach
from scripts.script_vm_observation import (
    compare_observation,
    require_tests,
)
from scripts.script_vm_policy import POLICY_STAGE, compare_case
from scripts.script_vm_provenance import (
    Spec,
    archive_files,
    digest,
    load_spec,
    verify_archive,
    verify_sources,
)
from scripts.script_vm_receipts import validate_receipts
from scripts.script_vm_roots import validate_roots
from scripts.script_vm_strings import validate_strings
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


def validate_layout(directory: Path, spec: Spec, *, packaged: bool) -> None:
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


def validate_matrix(
    directory: Path, spec: Spec, identities: tuple[str, str]
) -> None:
    origin, rust_path = identities
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
                if side == "native" and stages[key] == POLICY_STAGE:
                    continue
                argv = [binary, str(Path(origin) / "inputs" / name), *map(str, credit)]
                if read_json(case / side / "argv.json") != argv:
                    raise WorldCheckError("VM matrix invocation differs")
            compare_case(source.read_bytes(), case, stages[key])


def validate(directory: Path, *, packaged: bool = True) -> None:
    spec = load_spec(MANIFEST)
    validate_layout(directory, spec, packaged=packaged)
    verify_sources(ROOT, directory / "native", spec)
    for name, sha in spec.sources:
        if digest(directory / "source" / name) != sha:
            raise WorldCheckError("Retained VM source changed")
    origin, rust_path = validate_receipts(directory, spec)
    validate_matrix(directory, spec, (origin, rust_path))
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
    validate_branches(directory)
    validate_strings(directory, Path(origin), rust_path)
    validate_constants(directory, Path(origin))

    validate_roots(directory, Path(origin))
    validate_arrays(directory, Path(origin))
    validate_foreach(directory, Path(origin))
    if read_json(directory / "summary.json") != {
        "cases": 10819,
        "fixtures": 1188,
        "credits": 9,
        "tests": 62,
        "strict_comparisons": 10666,
        "undefined_input_rejections": 153,
        "controls": 25,
        "frame_tests": 34,
        "native_frames": 12,
        "branch_budget_cases": 127,
        "string_sessions": 4,
        "source_feed_cases": 18,
        "float_patterns": 23071,
        "string_controls": 4,
        "constant_native_sessions": 115,
        "constant_buffer_comparisons": 37,
        "constant_root_runtime_errors": 1,
        "constant_stateful_projections": 3,
        "constant_public_tests": 3,
        "constant_controls": 4,
        "root_native_sessions": 90,
        "root_stateful_projections": 75,
        "root_syntax_boundaries": 9,
        "root_runtime_boundaries": 6,
        "root_public_tests": 7,
        "root_controls": 4,
        "array_native_sessions": 93,
        "array_stateful_projections": 78,
        "array_boundary_sessions": 12,
        "array_protocol_refusals": 3,
        "array_public_tests": 11,
        "array_controls": 4,
        "foreach_native_sessions": 255,
        "foreach_stateful_projections": 255,
        "foreach_runtime_boundaries": 0,
        "foreach_public_tests": 19,
        "foreach_controls": 15,
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
