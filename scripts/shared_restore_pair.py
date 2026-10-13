from __future__ import annotations

from pathlib import Path

from scripts.backup_sale_run import member
from scripts.gameplay_foundations import digest, require_test
from scripts.grf_control_evidence import sequence, text
from scripts.owned_restore_exports import export
from scripts.owned_restore_run import RestoreRun
from scripts.world_check_support import WorldCheckError, at, read_json, run


def pair(job: RestoreRun, name: str) -> None:
    case = member(job.output / "results/cases", name)
    inputs = read_json(case / "inputs.json")
    source = text(at(inputs, ("source",)))
    descriptor = read_json(case / "actions.json")
    result = run(
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
        case / "rust-command",
    )
    require_test(result.stdout, job.selector)
    cli = job.executable("cli")
    initial = export(job, Path(source), case / "initial-export")
    _ = (case / "initial-native.json").write_text(initial)
    saves = [
        text(at(action, ("label",)))
        for action in sequence(at(descriptor, ("actions",)))
        if at(action, ("op",)) == "save"
    ]
    if not saves or len(set(saves)) != len(saves):
        raise WorldCheckError("Restore requires unique saved checkpoints")
    for directory in (case / "original/native", case / "rust"):
        if {path.stem for path in directory.glob("*.sav")} != set(saves):
            raise WorldCheckError("Restore save omitted or added")
    for label in saves:
        for kind, directory in (
            ("native", case / "original/native"),
            ("rust", case / "rust"),
        ):
            exported = export(
                job, directory / f"{label}.sav", case / f"{label}-{kind}-export"
            )
            _ = (case / f"{label}-{kind}.json").write_text(exported)
        _ = run(
            [
                cli,
                "compare",
                str(case / f"{label}-rust.json"),
                str(case / f"rust/{label}.world.json"),
            ],
            case / f"{label}-selfdecode",
        )
        _ = run(
            [
                "python3",
                "-m",
                "scripts.shared_restore_compare",
                "--plan",
                str(case / "actions.json"),
                "--native",
                str(case / "original/native/results.json"),
                "--rust",
                str(case / "rust/results.json"),
                "--initial-native",
                str(case / "initial-native.json"),
                "--initial-rust",
                str(case / "rust/initial.world.json"),
                "--label",
                label,
                "--expected",
                str(case / f"{label}-native.json"),
                "--actual",
                str(case / f"{label}-rust.json"),
                "--ledger",
                str(case / f"{label}-ledger.json"),
            ],
            case / f"{label}-compare",
        )
    _ = run(
        [
            cli,
            "compare",
            str(case / "original/saved-world.json"),
            str(case / f"{saves[-1]}-native.json"),
        ],
        case / "serializer-compare",
    )
    if digest(case / "actions.json") != at(inputs, ("descriptor_sha256",)):
        raise WorldCheckError("Restore descriptor changed")
    print(f"PASS original shared Restore {name}", flush=True)
