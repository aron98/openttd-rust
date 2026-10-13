from __future__ import annotations

from scripts.backup_sale_run import member
from scripts.grf_control_evidence import sequence, text
from scripts.owned_restore_controls import mutate as original_mutate
from scripts.owned_restore_controls import mutation_path
from scripts.owned_restore_run import RestoreRun
from scripts.purchase_creation import integer
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    read_json,
    replace,
    run,
    write_json,
)


def mutate(documents: Json, change: Json) -> Json:
    if at(change, ("kind",)) != "xor":
        return original_mutate(documents, change)
    document = at(documents, (text(at(change, ("document",))),))
    path = mutation_path(at(change, ("path",)))
    before = integer(at(document, path))
    value = before ^ integer(at(change, ("value",)))
    if value == before:
        raise WorldCheckError("Shared mutation is ineffective")
    replace(document, path, value)
    return before


def semantic_controls(job: RestoreRun) -> None:
    recipes = read_json(job.root / "scripts/shared-restore-mutations.json")
    entries = sequence(at(recipes, ("controls",)))
    names = [text(at(entry, ("name",))) for entry in entries]
    if len(names) != 22 or len(set(names)) != 22:
        raise WorldCheckError("Restore semantic control membership differs")
    for entry in entries:
        directory = member(job.output / "controls/semantic", text(at(entry, ("name",))))
        directory.mkdir(parents=True)
        case = member(job.output / "results/cases", text(at(entry, ("case",))))
        label = text(at(entry, ("label",)))
        files = {
            "plan": "actions.json",
            "native": "original/native/results.json",
            "rust": "rust/results.json",
            "initial-native": "initial-native.json",
            "initial-rust": "rust/initial.world.json",
            "expected": f"{label}-native.json",
            "actual": f"{label}-rust.json",
        }
        documents: dict[str, Json] = {
            key: read_json(member(case, name)) for key, name in files.items()
        }
        before = [
            mutate(documents, change) for change in sequence(at(entry, ("mutations",)))
        ]
        write_json(directory / "mutation.json", {"recipe": entry, "before": before})
        changed = {
            text(at(change, ("document",)))
            for change in sequence(at(entry, ("mutations",)))
        }
        argv = ["python3", "-m", "scripts.shared_restore_compare"]
        for key, value in documents.items():
            path = member(case, files[key])
            if key in changed:
                path = directory / f"{key}.json"
                write_json(path, value)
            argv.extend(["--" + key, str(path)])
        argv.extend(["--label", label, "--ledger", str(directory / "ledger.json")])
        result = run(argv, directory / "command", 1)
        if text(at(entry, ("diagnostic",))) not in result.stderr:
            raise WorldCheckError("Restore semantic rejection diagnostic differs")
