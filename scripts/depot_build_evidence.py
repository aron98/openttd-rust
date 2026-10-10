from __future__ import annotations

from pathlib import Path

from scripts.depot_build_archive import bounded_paths, package_raw
from scripts.depot_build_provenance import verify
from scripts.gameplay_foundations import digest
from scripts.replay_matrix import checkpoint_labels, deterministic
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json

LAYOUT = Path(__file__).with_name("depot-build-evidence-layout.json")


def strings(value: Json) -> tuple[str, ...]:
    match value:
        case list() as entries if all(isinstance(entry, str) for entry in entries):
            values = tuple(entry for entry in entries if isinstance(entry, str))
            if len(set(values)) != len(values):
                raise WorldCheckError("Duplicate depot evidence identity")
            return values
        case _:
            raise WorldCheckError("Invalid depot evidence identities")


def validate_cases(output: Path, layout: Json) -> tuple[str, ...]:
    cases = strings(at(layout, ("cases",)))
    if (
        len(cases) != 178
        or at(read_json(output / "inputs/manifest.json"), ("cases",)) != list(cases)
        or read_json(output / "results/summary.json") != {"passed": list(cases)}
    ):
        raise WorldCheckError("Incomplete depot matrix; subsets are not admitted")
    actions = commands = executions = 0
    for name in (*cases, "continuation/prefix", "continuation/suffix"):
        case = output / "results" / name
        plan = (
            read_json(output / "inputs" / f"{name}.json")
            if name in cases
            else read_json(output / "results" / f"{name}.json")
        )
        if plan != at(layout, ("plans", name)):
            raise WorldCheckError(f"Depot action/checkpoint identities changed: {name}")
        expected = at(plan, ("actions",))
        native = read_json(case / "native/results.json")
        rust = read_json(case / "rust/results.json")
        if deterministic(native) != rust:
            raise WorldCheckError(f"Depot command receipts differ: {name}")
        match expected:
            case list():
                labels = [
                    "initial",
                    *[
                        at(action, ("label",))
                        for action in expected
                        if at(action, ("op",)) == "checkpoint"
                    ],
                    "final",
                ]
            case _:
                raise WorldCheckError("Missing depot action list")
        if checkpoint_labels(native) != labels or checkpoint_labels(rust) != labels:
            raise WorldCheckError("Depot checkpoint identity changed")
        observed = at(native, ("actions",))
        match observed:
            case list() if len(observed) == len(expected):
                pass
            case _:
                raise WorldCheckError("Depot observed action coverage changed")
        if name not in cases:
            continue
        actions += len(expected)
        for request, result in zip(expected, observed, strict=True):
            if at(request, ("op",)) != "command":
                continue
            commands += 1
            execution = at(result, ("receipt", "exec"))
            if execution is not None and at(execution, ("success",)) is True:
                executions += 1
    if (actions, commands, executions) != (1352, 676, 201):
        raise WorldCheckError("Depot coverage differs from full native matrix")
    return cases


def validate_receipts(output: Path, paths: list[Path], layout: Json) -> None:
    controls = strings(at(layout, ("controls",)))
    failures = {f"controls/{name}/compare/process.json" for name in controls}
    receipts = [path for path in paths if path.name == "process.json"]
    if len(receipts) != 7189 or len(controls) != 7:
        raise WorldCheckError("Depot process/control coverage changed")
    for path in receipts:
        status = int(str(path.relative_to(output / "results")) in failures)
        if read_json(path) != {"returncode": status, "expected": status}:
            raise WorldCheckError(f"Depot process status differs: {path}")
    for name in controls:
        directory = output / "results/controls" / name
        assertion = read_json(directory / "assertion.json")
        match assertion:
            case {
                "path": str() as field,
                "expected": int() as old,
                "actual": int() as new,
                "rejected": True,
            } if type(old) is int and type(new) is int and new == old + 1:
                error = (directory / "compare/stderr.log").read_text()
                if field != at(layout, ("control_paths", name)) or any(
                    text not in error for text in (field, str(old), str(new))
                ):
                    raise WorldCheckError(
                        "Depot negative control failed for wrong reason"
                    )
            case _:
                raise WorldCheckError("Missing depot control rejection")


def validate_native(output: Path) -> None:
    identity = read_json(output / "provenance.json")
    count = 0
    for path in output.rglob("invocation.txt"):
        data = dict(
            line.split("=", 1)
            for line in path.read_text().splitlines()
            if "=" in line and not line.startswith("SOURCE_SHA256=")
        )
        if data.get("ORACLE") != at(identity, ("oracle",)) or data.get(
            "ORACLE_SHA256"
        ) != at(identity, ("oracle_sha256",)):
            raise WorldCheckError("Native depot executable identity changed")
        for filename, sha in (("INPUT", "INPUT_SHA256"), ("REPLAY", "REPLAY_SHA256")):
            if filename == "REPLAY" and path.parent == output / "original-depots":
                continue
            if (
                filename not in data
                or sha not in data
                or digest(Path(data[filename])) != data[sha]
            ):
                raise WorldCheckError("Native depot input identity differs")
        count += 1
    if count != 542:
        raise WorldCheckError("Native depot invocation coverage changed")
    for path in output.rglob("process.txt"):
        if path.read_text() != "exit=0\n":
            raise WorldCheckError("Native depot process did not succeed")


def package(root: Path, output: Path) -> None:
    layout = read_json(LAYOUT)
    paths = bounded_paths(output / "results", set(strings(at(layout, ("paths",)))))
    cases = validate_cases(output, layout)
    validate_receipts(output, paths, layout)
    verify(root, output, cases)
    validate_native(output)
    write_json(
        output / "coverage.json",
        {
            "cases": list(cases),
            "actions": 1352,
            "commands": 676,
            "executions": 201,
            "controls": list(strings(at(layout, ("controls",)))),
            "runtime_invocations": 180,
            "process_receipts": 7189,
            "layout_sha256": digest(LAYOUT),
        },
    )
    _ = bounded_paths(
        output,
        {"results/" + name for name in strings(at(layout, ("paths",)))}
        | set(strings(at(layout, ("setup_paths",)))),
    )
    package_raw(output)
