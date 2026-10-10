from __future__ import annotations

from scripts.backup_sale_bindings import edit_job, native_job, prepare_evidence
from scripts.backup_sale_run import SELECTOR, BackupSaleRun, member
from scripts.context_ci_support import exact
from scripts.depot_build_archive import bounded_paths
from scripts.depot_removal_pair import projection
from scripts.gameplay_foundations import require_test
from scripts.grf_control_evidence import sequence, text
from scripts.order_state_evidence import command
from scripts.world_check_support import Json, WorldCheckError, at, read_json


def membership(actual: Json, expected: Json) -> None:
    if not exact(actual, expected) or not sequence(expected):
        raise WorldCheckError("Exact backup sale case membership required")


def witnesses(native: Json, expected: Json) -> None:
    actual: list[Json] = [
        {"index": at(row, ("index",)), "receipt": at(row, ("result", "receipt"))}
        for row in sequence(at(native, ("actions",)))
        if at(row, ("input", "op")) == "command"
    ]
    membership(actual, expected)


def pair_evidence(job: BackupSaleRun, name: str) -> tuple[int, int]:
    case = member(job.output / "results", name)
    inputs = read_json(case / "inputs.json")
    descriptor = read_json(case / "actions.json")
    source = text(at(inputs, ("source",)))
    runner, cli = job.executable("runner"), job.executable("cli")
    command(
        case / "rust-command",
        [
            "env",
            f"BACKUP_SALE_INPUT={source}",
            f"BACKUP_SALE_ACTIONS={case}/actions.json",
            f"BACKUP_SALE_OUTPUT={case}/rust",
            runner,
            "--exact",
            SELECTOR,
            "--ignored",
            "--nocapture",
        ],
    )
    require_test((case / "rust-command/stdout.log").read_text(), SELECTOR)
    expected = projection(read_json(case / "original/native/results.json"), descriptor)
    actual = read_json(case / "rust/results.json")
    if not exact(expected, actual) or not exact(
        expected, read_json(case / "native-state.json")
    ):
        raise WorldCheckError("Complete depot command state differs")
    command(
        case / "live-compare",
        [
            cli,
            "compare",
            str(case / "native-state.json"),
            str(case / "rust/results.json"),
        ],
    )
    actions = sequence(at(descriptor, ("actions",)))
    labels = [text(at(a, ("label",))) for a in actions if at(a, ("op",)) == "save"]
    if not labels or len(set(labels)) != len(labels):
        raise WorldCheckError("Missing exact saved checkpoint membership")
    for folder in (case / "original/native", case / "rust"):
        if {p.stem for p in folder.glob("*.sav")} != set(labels):
            raise WorldCheckError("Depot checkpoint file membership differs")
    for label in labels:
        for kind, folder in (
            ("native", case / "original/native"),
            ("rust", case / "rust"),
        ):
            log = case / f"{label}-{kind}-export"
            command(
                log, [cli, "world", str(folder / f"{label}.sav"), "--view", "saved"]
            )
            if (log / "stdout.log").read_bytes() != (
                case / f"{label}-{kind}.json"
            ).read_bytes():
                raise WorldCheckError("Actual saved export changed")
        if not exact(
            read_json(case / f"{label}-native.json"),
            read_json(case / f"{label}-rust.json"),
        ):
            raise WorldCheckError("Full native/Rust saved state differs")
        command(
            case / f"{label}-compare",
            [
                cli,
                "compare",
                str(case / f"{label}-native.json"),
                str(case / f"{label}-rust.json"),
            ],
        )
    command(
        case / "serializer-compare",
        [
            cli,
            "compare",
            str(case / "original/saved-world.json"),
            str(case / f"{labels[-1]}-rust.json"),
        ],
    )
    if not exact(
        read_json(case / "original/saved-world.json"),
        read_json(case / f"{labels[-1]}-rust.json"),
    ):
        raise WorldCheckError("Original serializer differs")
    return len(actions), len(labels)


def validate(job: BackupSaleRun, layout: Json) -> Json:
    prepare_evidence(job)
    bridge_witness(
        read_json(
            job.output / "results/matrix/copy-sale-clear/original/native/results.json"
        )
    )
    membership(at(job.fixtures, ("cases",)), at(layout, ("cases",)))
    cases = [text(v) for v in sequence(at(layout, ("cases",)))]
    membership(
        [at(entry, ("directory",)) for entry in sequence(at(job.fixtures, ("jobs",)))],
        at(layout, ("jobs",)),
    )
    for entry in sequence(at(job.fixtures, ("jobs",))):
        match at(entry, ("kind",)):
            case "native":
                native_job(job, entry)
            case "edit":
                edit_job(job, entry)
            case _:
                raise WorldCheckError("Unknown fixture admission operation")
    actions = saves = 0
    for name in cases:
        witnesses(
            read_json(
                member(job.output / "results", name) / "original/native/results.json"
            ),
            at(layout, ("command_witnesses", name)),
        )
        count, saved = pair_evidence(job, name)
        actions += count
        saves += saved
    coverage: Json = {
        "cases": len(cases),
        "actions": actions,
        "snapshots": len(cases) * 2 + actions * 2,
        "saves": saves,
    }
    if not exact(coverage, at(layout, ("coverage",))):
        raise WorldCheckError("Backup sale executed coverage differs")
    _ = bounded_paths(
        job.output / "results",
        {text(v) for v in sequence(at(layout, ("result_paths",)))},
    )
    return coverage


def bridge_witness(native: Json) -> None:
    before = at(native, ("actions", 4, "after", "orders", "backups"))
    sold = at(native, ("actions", 5, "after", "orders", "backups"))
    if not exact(before, sold) or len(sequence(before)) != 4:
        raise WorldCheckError("Copied backups did not survive sale unchanged")
    for row in sequence(before):
        if at(row, ("clone",)) is not None or not sequence(at(row, ("orders",))):
            raise WorldCheckError("Same-tile witness requires owned copied orders")
    remaining = at(native, ("actions", 7, "after", "orders", "backups"))
    if not exact(remaining, [sequence(before)[2]]):
        raise WorldCheckError(
            "Depot cleanup did not distinguish same-tile and referenced backups"
        )
    expected: Json = {
        "first_free": 0,
        "first_unused": 4,
        "items": 1,
        "slots": 4,
        "occupied": [2],
    }
    if not exact(
        at(native, ("actions", 7, "after", "orders", "backup_pool")), expected
    ):
        raise WorldCheckError("Live backup pool history differs after depot clear")
