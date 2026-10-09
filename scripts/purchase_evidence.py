from __future__ import annotations

import hashlib
import sys
import tarfile
from pathlib import Path

if __package__ in (None, ""):
    sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.gameplay_foundations import require_test
from scripts.grf_metadata_evidence import digest
from scripts.purchase_provenance import verify
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json

CASES = (
    "temperate-original",
    "temperate-realistic",
    "arctic",
    "tropic",
    "toyland",
    "limit",
    "money",
    "exact-money",
    "zero-cost-negative-cash",
    "nonowner",
    "unavailable",
    "wrong-depot",
    "pause",
    "dynamic",
    "legacy-paid",
    "errors",
)
CONTROLS = (
    "vehicle-id",
    "cargo-array",
    "cost",
    "expense",
    "rng",
    "vehicle-random",
    "service-date",
    "orders-reference",
    "pool-cursor",
    "unit-number",
    "creation-power",
)


def paths_from_layout(layout: Json) -> set[str]:
    match layout:
        case list() as entries if all(isinstance(value, str) for value in entries):
            paths = {value for value in entries if isinstance(value, str)}
            if len(paths) != len(entries):
                raise WorldCheckError("Duplicate purchase evidence path")
            return paths
        case _:
            raise WorldCheckError("Invalid purchase evidence layout")


def validate_paths(root: Path, expected: set[str]) -> list[Path]:
    actual = {str(path.relative_to(root)) for path in root.rglob("*") if path.is_file()}
    if actual != expected:
        raise WorldCheckError(
            "Purchase evidence path set differs from the complete matrix"
        )
    paths = [root / name for name in sorted(expected)]
    for path in paths:
        if not path.resolve().is_relative_to(root.resolve()) or path.is_symlink():
            raise WorldCheckError("Purchase evidence escaped its artifact directory")
        if path.stat().st_size == 0 and path.name not in ("stdout.log", "stderr.log"):
            raise WorldCheckError(f"Empty purchase evidence: {path}")
    return paths


def validate_receipts(root: Path, paths: list[Path]) -> None:
    receipts = [path for path in paths if path.name == "process.json"]
    if len(receipts) != 869:
        raise WorldCheckError("Purchase process receipt count changed")
    failures = {f"controls/{name}/compare/process.json" for name in CONTROLS}
    for path in receipts:
        expected = int(str(path.relative_to(root)) in failures)
        if read_json(path) != {"returncode": expected, "expected": expected}:
            raise WorldCheckError(
                f"Purchase process did not complete as required: {path}"
            )
    require_test(
        (root / "setup-command/stdout.log").read_text(), "prepare_purchase_inputs"
    )
    for name in (*CASES, "split/prefix", "split/suffix"):
        require_test(
            (root / name / "rust-command/stdout.log").read_text(),
            "runtime::purchase_native::run_native_purchase_sequence",
        )
    for name in CONTROLS:
        assertion = read_json(root / "controls" / name / "assertion.json")
        if at(assertion, ("rejected",)) is not True:
            raise WorldCheckError(f"Purchase corruption was not rejected: {name}")


def validate_cases(root: Path) -> None:
    summary = read_json(root / "summary.json")
    if at(summary, ("passed",)) is not True or at(summary, ("cases",)) != list(CASES):
        raise WorldCheckError("Incomplete purchase cases")
    total_actions = 0
    total_commands = 0
    successful = 0
    engines: set[int] = set()
    for name in CASES:
        actions = at(read_json(root / name / "actions.json"), ("actions",))
        match actions:
            case list():
                total_actions += len(actions)
                total_commands += sum(
                    at(action, ("op",)) == "command" for action in actions
                )
            case _:
                raise WorldCheckError("Missing purchase actions")
        observed = at(read_json(root / name / "native/results.json"), ("actions",))
        match observed:
            case list() if len(observed) == len(actions):
                for request, result in zip(actions, observed, strict=True):
                    if at(request, ("op",)) != "command":
                        continue
                    execution = at(result, ("receipt", "exec"))
                    if execution is None or at(execution, ("success",)) is not True:
                        continue
                    successful += 1
                    engine = at(request, ("request", "command", "engine"))
                    match engine:
                        case int() if not isinstance(engine, bool):
                            engines.add(engine)
                        case _:
                            raise WorldCheckError("Invalid purchased engine identity")
            case _:
                raise WorldCheckError("Native purchase action coverage differs")
    if (total_actions, total_commands, successful, len(engines)) != (345, 260, 194, 88):
        raise WorldCheckError("Purchase action coverage changed")


def archive_files(directory: Path, paths: list[Path]) -> None:
    destination = directory / "evidence.tar.gz"
    hashes = {str(path.relative_to(directory)): digest(path) for path in paths}
    with tarfile.open(destination, "x:gz") as archive:
        for path in paths:
            archive.add(path, arcname=str(path.relative_to(directory)), recursive=False)
    observed: set[str] = set()
    with tarfile.open(destination, "r:gz") as archive:
        for member in archive:
            if (
                not member.isfile()
                or member.name not in hashes
                or member.name in observed
            ):
                raise WorldCheckError("Purchase archive membership changed")
            source = archive.extractfile(member)
            if source is None:
                raise WorldCheckError("Purchase archive data missing")
            with source:
                actual = hashlib.sha256(source.read()).hexdigest()
            if actual != hashes[member.name]:
                raise WorldCheckError("Purchase archive content changed")
            observed.add(member.name)
    if observed != set(hashes):
        raise WorldCheckError("Purchase archive is incomplete")
    indexed: dict[str, Json] = dict(hashes)
    write_json(
        directory / "evidence-index.json",
        {
            "schema_version": 1,
            "files": indexed,
            "count": len(paths),
            "archive_sha256": digest(destination),
        },
    )


def package(directory: Path) -> None:
    root = directory / "results"
    layout = read_json(Path(__file__).with_name("purchase-evidence-layout.json"))
    expected = paths_from_layout(layout)
    paths = validate_paths(root, expected)
    validate_cases(root)
    validate_receipts(root, paths)
    verify(directory, (*CASES, "split/prefix", "split/suffix"))
    archive_files(directory, paths)


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise WorldCheckError("Expected a fresh purchase evidence directory")
    package(Path(sys.argv[1]).resolve())
