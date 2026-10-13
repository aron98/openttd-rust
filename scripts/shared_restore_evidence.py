from __future__ import annotations

from pathlib import Path

from scripts.backup_sale_run import member
from scripts.context_ci_support import exact
from scripts.gameplay_foundations import require_test
from scripts.grf_control_evidence import sequence, text
from scripts.order_state_evidence import command
from scripts.owned_restore_exports import verify_export
from scripts.owned_restore_native_evidence import native_receipt
from scripts.owned_restore_run import RestoreRun
from scripts.shared_restore_compare import Comparison, compare
from scripts.shared_restore_setup_evidence import validate_preparation
from scripts.shared_restore_units import SELECTORS, validate_units
from scripts.world_check_support import Json, WorldCheckError, at, read_json


def case_bindings(job: RestoreRun, entry: Json) -> Path:
    name = text(at(entry, ("name",)))
    case = member(job.output / "results/cases", name)
    source = member(job.output / "results", text(at(entry, ("source",))))
    descriptor = at(entry, ("descriptor",))
    if not exact(read_json(case / "actions.json"), descriptor):
        raise WorldCheckError("Restore descriptor membership differs")
    native_receipt(job, case, source)
    command(
        case / "rust-command",
        [
            "env",
            f"OWNED_RESTORE_INPUT={source}",
            f"OWNED_RESTORE_ACTIONS={case}/actions.json",
            f"OWNED_RESTORE_OUTPUT={case}/rust",
            job.executable("runner"),
            "--exact",
            job.selector,
            "--ignored",
            "--nocapture",
        ],
    )
    require_test((case / "rust-command/stdout.log").read_text(), job.selector)
    initial = verify_export(job, source, case / "initial-export")
    if not exact(initial, read_json(case / "initial-native.json")):
        raise WorldCheckError("Restore initial decoded output differs")
    return case


def save_membership(directory: Path, labels: list[str]) -> None:
    if {path.stem for path in directory.glob("*.sav")} != set(labels):
        raise WorldCheckError("Restore save membership differs")


def validate_case(job: RestoreRun, entry: Json) -> Json:
    case = case_bindings(job, entry)
    name = text(at(entry, ("name",)))
    descriptor = at(entry, ("descriptor",))
    native_directory = case / "original"
    native = read_json(native_directory / "native/results.json")
    rust = read_json(case / "rust/results.json")
    saves = [
        text(at(action, ("label",)))
        for action in sequence(at(descriptor, ("actions",)))
        if at(action, ("op",)) == "save"
    ]
    if not saves or len(saves) != len(set(saves)):
        raise WorldCheckError("Restore checkpoint labels differ")
    for directory in (native_directory / "native", case / "rust"):
        save_membership(directory, saves)
    for label in saves:
        for kind, directory in (
            ("native", native_directory / "native"),
            ("rust", case / "rust"),
        ):
            decoded = verify_export(
                job, directory / f"{label}.sav", case / f"{label}-{kind}-export"
            )
            if not exact(decoded, read_json(case / f"{label}-{kind}.json")):
                raise WorldCheckError("Restore saved decoded output differs")
        proof = Comparison(
            descriptor,
            native,
            rust,
            read_json(case / "initial-native.json"),
            read_json(case / "rust/initial.world.json"),
            label,
            read_json(case / f"{label}-native.json"),
            read_json(case / f"{label}-rust.json"),
        )
        ledger = compare(proof)
        if not exact(ledger, read_json(case / f"{label}-ledger.json")):
            raise WorldCheckError("Restore lifetime ledger differs")
        if not exact(proof.actual, read_json(case / f"rust/{label}.world.json")):
            raise WorldCheckError("Restore Rust save selfdecode differs")
    if not exact(
        read_json(native_directory / "saved-world.json"),
        read_json(case / f"{saves[-1]}-native.json"),
    ):
        raise WorldCheckError("Restore native serializer differs")
    actions = len(sequence(at(descriptor, ("actions",))))
    return {
        "case": name,
        "actions": actions,
        "snapshots": 2 + 2 * actions,
        "saves": len(saves),
    }


def validate(job: RestoreRun) -> Json:
    entries = sequence(at(job.fixtures, ("cases",)))
    names = [text(at(entry, ("name",))) for entry in entries]
    actual = {path.name for path in (job.output / "results/cases").iterdir()}
    if len(names) != 16 or len(set(names)) != 16 or actual != set(names):
        raise WorldCheckError("Restore exact 16-case membership differs")
    validate_units(job)
    validate_preparation(job)
    rows = [validate_case(job, entry) for entry in entries]
    totals: dict[str, Json] = {"cases": len(rows)}
    for field in ("actions", "snapshots", "saves"):
        total = 0
        for row in rows:
            match at(row, (field,)):
                case int() as value:
                    total += value
                case _:
                    raise WorldCheckError("Invalid Restore coverage count")
        totals[field] = total
    if not exact(totals, at(job.fixtures, ("coverage",))):
        raise WorldCheckError("Restore full coverage differs")
    return {
        "coverage": totals,
        "cases": rows,
        "executed_rust_tests": len(SELECTORS) + len(rows),
    }
