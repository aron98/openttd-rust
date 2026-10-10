from __future__ import annotations

from scripts.backup_sale_run import member
from scripts.context_ci_support import exact
from scripts.grf_control_evidence import sequence, text
from scripts.owned_restore_run import RestoreRun
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    read_json,
    replace,
    run,
    write_json,
)


def mutation_path(value: Json) -> tuple[str | int, ...]:
    path: list[str | int] = []
    for part in sequence(value):
        match part:
            case str() | int():
                path.append(part)
            case _:
                raise WorldCheckError("Invalid Restore mutation path")
    if not path:
        raise WorldCheckError("Restore mutation cannot replace whole document")
    return tuple(path)


def mutate(documents: Json, change: Json) -> Json:
    document = at(documents, (text(at(change, ("document",))),))
    path = mutation_path(at(change, ("path",)))
    match at(change, ("kind",)):
        case "replace":
            before = at(document, tuple(path))
            value = at(change, ("value",))
            if exact(before, value):
                raise WorldCheckError("Restore mutation is ineffective")
            replace(document, tuple(path), value)
        case "add" | "remove" as kind:
            parent = at(document, tuple(path[:-1]))
            match parent, path[-1]:
                case dict() as fields, str() as key:
                    if kind == "add":
                        if key in fields:
                            raise WorldCheckError(
                                "Restore added mutation already exists"
                            )
                        before = None
                        fields[key] = at(change, ("value",))
                    else:
                        if key not in fields:
                            raise WorldCheckError("Restore removed mutation absent")
                        before = fields.pop(key)
                case _:
                    raise WorldCheckError("Restore membership mutation requires object")
        case _:
            raise WorldCheckError("Unknown Restore mutation")
    return before


def semantic_controls(job: RestoreRun) -> None:
    recipes = read_json(job.root / "scripts/owned-restore-mutations.json")
    entries = sequence(at(recipes, ("controls",)))
    names = [text(at(entry, ("name",))) for entry in entries]
    if len(names) != 18 or len(set(names)) != 18:
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
        argv = ["python3", "-m", "scripts.owned_restore_saved_state"]
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
