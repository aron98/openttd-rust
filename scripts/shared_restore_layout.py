from __future__ import annotations

import ast
from pathlib import Path
from typing import Final

from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import sequence, text
from scripts.owned_restore_sources import compiled_inputs, regular
from scripts.shared_restore_guards import cases
from scripts.shared_restore_units import SELECTORS
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json

COMMAND: Final = ("argv.json", "process.json", "stdout.log", "stderr.log")


def command(paths: set[str], prefix: str, *, bindings: bool = False) -> None:
    paths.update(prefix + "/" + name for name in COMMAND)
    if bindings:
        paths.add(prefix + "/bindings.json")


def native(paths: set[str], prefix: str, descriptor: Json) -> None:
    paths.update(prefix + "/" + name for name in ("actions.json", "inputs.json"))
    names = (
        *COMMAND,
        "bindings-before.json",
        "bindings-after.json",
        "environment.json",
        "loaded.json",
        "saved-world.json",
        "saved-schema.json",
        "openttd.cfg",
        "native/results.json",
    )
    paths.update(prefix + "/original/" + name for name in names)
    for action in sequence(at(descriptor, ("actions",))):
        if at(action, ("op",)) == "save":
            paths.add(
                prefix + "/original/native/" + text(at(action, ("label",))) + ".sav"
            )


def result_paths(fixtures: Json) -> set[str]:
    paths = {
        "matrix/prepared/" + name
        for name in (
            "depot-runtime.json",
            "invocation.txt",
            "openttd.cfg",
            "save/autosave/exit.sav",
            "stderr.log",
            "stdout.log",
        )
    }
    command(paths, "prepare-command")
    for entry in sequence(at(fixtures, ("jobs",))):
        prefix = text(at(entry, ("directory",)))
        match at(entry, ("kind",)):
            case "native":
                native(paths, prefix, at(entry, ("descriptor",)))
            case "edit":
                output = Path(text(at(entry, ("output",))))
                paths.update((str(output), str(output.with_suffix(".edit.json"))))
                command(paths, prefix, bindings=True)
            case _:
                raise WorldCheckError("Unlisted shared preparation kind")
    paths.add("preparation/capacity.sav")
    paths.add("capacity-preparation.json")
    native(paths, "preparation/capacity-load", at(fixtures, ("capacity_load",)))
    for phase in ("original", "prepared", "resaved"):
        paths.add("capacity-" + phase + ".json")
        command(paths, "capacity-" + phase + "-export", bindings=True)
    for entry in sequence(at(fixtures, ("cases",))):
        prefix = "cases/" + text(at(entry, ("name",)))
        descriptor = at(entry, ("descriptor",))
        native(paths, prefix, descriptor)
        command(paths, prefix + "/rust-command")
        command(paths, prefix + "/initial-export", bindings=True)
        paths.update(
            prefix + "/" + name
            for name in (
                "initial-native.json",
                "rust/results.json",
                "rust/initial.world.json",
            )
        )
        for action in sequence(at(descriptor, ("actions",))):
            if at(action, ("op",)) != "save":
                continue
            label = text(at(action, ("label",)))
            paths.update(
                prefix + "/" + name
                for name in (
                    f"{label}-native.json",
                    f"{label}-rust.json",
                    f"{label}-ledger.json",
                    f"rust/{label}.sav",
                    f"rust/{label}.world.json",
                )
            )
            for kind in ("native", "rust"):
                command(paths, prefix + f"/{label}-{kind}-export", bindings=True)
            for kind in ("selfdecode", "compare"):
                command(paths, prefix + f"/{label}-{kind}")
        command(paths, prefix + "/serializer-compare")
    return paths


def control_paths(mutations: Json, sources: tuple[str, ...]) -> set[str]:
    paths: set[str] = set()
    for name, *_ in cases():
        paths.update(
            "native/" + name + "/" + field
            for field in (
                "actions.json",
                "loaded.json",
                "bindings-before.json",
                "bindings-after.json",
            )
        )
        command(paths, "native/" + name + "/command")
    for entry in sequence(at(mutations, ("controls",))):
        prefix = "semantic/" + text(at(entry, ("name",)))
        paths.add(prefix + "/mutation.json")
        command(paths, prefix + "/command")
        paths.update(
            prefix + "/" + text(at(change, ("document",))) + ".json"
            for change in sequence(at(entry, ("mutations",)))
        )
    paths.update(
        "native/" + name for name in ("openttd", "replay-build.sha256", "mutation.json")
    )
    for phase in ("before", "rejected"):
        command(paths, "native/" + phase)
    paths.update(
        (
            "admission/results.json",
            "admission/wrong-source-mutation.json",
            "admission/wrong-executable.json",
            "admission/extra-save/after.sav",
            "admission/extra-save/extra.sav",
        )
    )
    for phase in ("zero-test", "subset-driver"):
        command(paths, "admission/" + phase)
    paths.update("admission/wrong-source/" + name for name in sources)
    paths.add("archive/bindings.json")
    for phase in ("baseline", "raw", "archive", "index"):
        paths.update(
            "archive/" + phase + "/" + name
            for name in (
                "actions.json",
                "after.sav",
                "rust.stdout.log",
                "evidence-index.json",
                "evidence.tar.gz",
            )
        )
        command(paths, "archive/commands/" + phase)
    return paths


def source_names(root: Path) -> tuple[str, ...]:
    required = set(compiled_inputs(root))
    required.update(regular(root, path) for path in root.glob("crates/**/*.rs"))
    required.update(
        (
            "upstream.toml",
            "rust-toolchain.toml",
            "scripts/reference.cfg",
            "scripts/setup-snapshot-reference.sh",
            "scripts/grf-control-ruff.toml",
            "scripts/shared-restore-fixtures.json",
            "scripts/shared-restore-mutations.json",
        )
    )
    required.update(
        regular(root, path) for path in (root / "reference").iterdir() if path.is_file()
    )
    required.update(regular(root, path) for path in (root / "scripts").glob("*.cmake"))
    pending = [
        root / "scripts/check-shared-restore.py",
        root / "scripts/shared_restore_layout.py",
        *root.glob("scripts/shared_restore_*test.py"),
    ]
    while pending:
        path = pending.pop()
        name = regular(root, path)
        if name in required:
            continue
        required.add(name)
        for node in ast.walk(ast.parse(path.read_text())):
            modules: list[str] = []
            if isinstance(node, ast.ImportFrom) and node.module:
                modules.append(node.module)
            if isinstance(node, ast.Import):
                modules.extend(alias.name for alias in node.names)
            pending.extend(
                root / (module.replace(".", "/") + ".py")
                for module in modules
                if module.startswith("scripts.")
            )
    return tuple(sorted(required))


def build_layout(root: Path) -> Json:
    fixtures = read_json(root / "scripts/shared-restore-fixtures.json")
    mutations = read_json(root / "scripts/shared-restore-mutations.json")
    names = source_names(root)
    base = {
        "bin/cli",
        "bin/runner",
        "binaries.json",
        "build-environment.json",
        "coverage.json",
        "layout.json",
        "native-bindings.json",
        "native-executable/openttd",
        "native-executable/replay-build.sha256",
        "provenance.json",
    }
    for name in (
        "cargo-cli",
        "cargo-runner",
        "freshness",
        "rust-version",
        "python-version",
        *("shared-unit-" + str(index) for index in range(len(SELECTORS))),
    ):
        command(base, "logs/" + name)
        base.add("logs/" + name + "/environment.json")
    pinned: dict[str, Json] = {name: digest(root / name) for name in names}
    units: list[Json] = list(SELECTORS)
    base_members: list[Json] = []
    base_members.extend(sorted(base))
    results: list[Json] = []
    results.extend(sorted(result_paths(fixtures)))
    controls: list[Json] = []
    controls.extend(sorted(control_paths(mutations, names)))
    result: dict[str, Json] = {
        "schema_version": 1,
        "sources": pinned,
        "coverage": at(fixtures, ("coverage",)),
        "units": units,
        "native_refusals": len(cases()),
        "semantic_controls": 22,
        "base_paths": base_members,
        "result_paths": results,
        "control_paths": controls,
    }
    return result


if __name__ == "__main__":
    project = Path(__file__).resolve().parents[1]
    write_json(project / "scripts/shared-restore-layout.json", build_layout(project))
