# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through scripts/check-order-state.py.
from __future__ import annotations

from pathlib import Path

from scripts.context_ci_support import exact
from scripts.gameplay_foundations import digest, require_test
from scripts.grf_control_evidence import sequence, text
from scripts.world_check_support import Json, WorldCheckError, at, read_json


def role_matches(value: Json, role: str) -> None:
    if at(value, ("networking",)) is not (role != "sp") or at(
        value, ("server",)
    ) is not (role == "server"):
        raise WorldCheckError("Native role does not match actual startup")


def projection(value: Json) -> Json:
    rows: list[Json] = []
    for row in sequence(at(value, ("actions",))):
        result = at(row, ("result",))
        if at(row, ("input", "op")) == "save":
            result = {key: at(result, (key,)) for key in ("path", "before", "after")}
        projected: dict[str, Json] = {
            key: at(row, (key,)) for key in ("index", "input", "before", "after")
        }
        projected["result"] = result
        rows.append(projected)
    output: dict[str, Json] = {
        key: at(value, (key,)) for key in ("schema_version", "case", "initial", "final")
    }
    output["actions"] = rows
    return output


def fields(value: Json, expected: set[str]) -> None:
    match value:
        case dict() as entries if set(entries) == expected:
            return
        case _:
            raise WorldCheckError("Order observation schema differs")


def compare_states(native: Json, rust: Json, descriptor: Json) -> None:
    fields(
        native,
        {"schema_version", "case", "role", "initial", "actions", "final", "final_role"},
    )
    fields(
        rust,
        {
            "schema_version",
            "case",
            "declared_context",
            "exit_requested",
            "load_receipt",
            "initial",
            "actions",
            "final",
        },
    )
    expected = sequence(at(descriptor, ("actions",)))
    if not expected:
        raise WorldCheckError("Empty order action membership")
    for value in (native, rust):
        rows = sequence(at(value, ("actions",)))
        if not exact([at(row, ("input",)) for row in rows], expected) or not exact(
            [at(row, ("index",)) for row in rows], list(range(len(expected)))
        ):
            raise WorldCheckError("Order action membership differs")
        if at(value, ("case",)) != at(descriptor, ("case",)) or not exact(
            at(value, ("schema_version",)), 1
        ):
            raise WorldCheckError("Order case identity differs")
    role = text(at(descriptor, ("role",)))
    role_matches(at(native, ("role",)), role)
    role_matches(at(native, ("final_role",)), role)
    for row in sequence(at(rust, ("actions",))):
        fields(row, {"index", "input", "before", "result", "after"})
    for row in sequence(at(native, ("actions",))):
        fields(row, {"index", "input", "before", "result", "after", "role"})
        role_matches(at(row, ("role",)), role)
        if at(row, ("input", "op")) == "save":
            role_matches(at(row, ("result", "role")), role)
    if not exact(projection(native), projection(rust)):
        raise WorldCheckError("Complete order state differs")


def validate_pair(case: Path, output: Path, descriptor: Json, manifest: Json) -> None:
    bindings(case, output, manifest)
    native = read_json(case / "original/native/results.json")
    rust = read_json(output / "rust/results.json")
    if not exact(read_json(case / "actions.json"), descriptor):
        raise WorldCheckError("Pinned case descriptor differs")
    compare_states(native, rust, descriptor)
    load_receipt(rust, read_json(output / "input-export/stdout.log"), descriptor)
    expected = {
        text(at(row, ("label",))) + ".sav"
        for row in sequence(at(descriptor, ("actions",)))
        if at(row, ("op",)) == "save"
    }
    for directory in (case / "original/native", output / "rust"):
        if {path.name for path in directory.glob("*.sav")} != expected:
            raise WorldCheckError("Saved checkpoint membership differs")
    for name in expected:
        stem = Path(name).stem
        if not exact(
            read_json(output / f"{stem}-native.json"),
            read_json(output / f"{stem}-rust.json"),
        ):
            raise WorldCheckError("Complete saved state differs")
    if not exact(
        read_json(case / "original/saved-world.json"),
        read_json(output / "after-rust.json"),
    ):
        raise WorldCheckError("Native serializer state differs")
    inputs = read_json(case / "inputs.json")
    if digest(Path(text(at(inputs, ("save",))))) != at(inputs, ("save_sha256",)):
        raise WorldCheckError("Order input changed")
    require_test(
        (output / "rust-command/stdout.log").read_text(),
        "runtime::order_state::native::original_order_state_case",
    )
    for path in output.rglob("process.json"):
        if at(read_json(path), ("returncode",)) != 0:
            raise WorldCheckError("Order comparison process failed")


def command(directory: Path, expected: list[str]) -> None:
    if not exact(read_json(directory / "argv.json"), list(expected)) or not exact(
        read_json(directory / "process.json"), {"returncode": 0, "expected": 0}
    ):
        raise WorldCheckError("Order command executable/arguments/status differ")


def bindings(case: Path, output: Path, manifest: Json) -> None:
    inputs = read_json(case / "inputs.json")
    source = text(at(inputs, ("save",)))
    runner = text(at(manifest, ("runner", "executable")))
    cli = text(at(manifest, ("cli", "executable")))
    command(output / "input-export", [cli, "world", source, "--view", "saved"])
    command(
        output / "rust-command",
        [
            "env",
            f"ORDER_CASE_INPUT={source}",
            f"ORDER_CASE_ACTIONS={case / 'actions.json'}",
            f"ORDER_CASE_OUTPUT={output / 'rust'}",
            runner,
            "--exact",
            text(at(manifest, ("case_test",))),
            "--ignored",
            "--nocapture",
        ],
    )
    for name in ("runner", "cli"):
        if digest(Path(text(at(manifest, (name, "executable"))))) != at(
            manifest, (name, "sha256")
        ):
            raise WorldCheckError("Executed order binary changed")
    command(
        output / "live-compare",
        [
            cli,
            "compare",
            str(output / "native-state.json"),
            str(output / "rust-state.json"),
        ],
    )
    for native in (case / "original/native").glob("*.sav"):
        stem = native.stem
        for label, source_path in (
            ("native", native),
            ("rust", output / "rust" / native.name),
        ):
            directory = output / f"{stem}-{label}-export"
            command(directory, [cli, "world", str(source_path), "--view", "saved"])
            if not exact(
                read_json(directory / "stdout.log"),
                read_json(output / f"{stem}-{label}.json"),
            ):
                raise WorldCheckError("Saved projection is not actual CLI output")
        command(
            output / f"{stem}-saved-compare",
            [
                cli,
                "compare",
                str(output / f"{stem}-native.json"),
                str(output / f"{stem}-rust.json"),
            ],
        )
    command(
        output / "native-serializer-compare",
        [
            cli,
            "compare",
            str(case / "original/saved-world.json"),
            str(output / "after-rust.json"),
        ],
    )


def load_receipt(rust: Json, saved: Json, descriptor: Json) -> None:
    match at(saved, ("chunks", "BKOR", "records")):
        case dict() as records:
            ids = sorted(int(key) for key in records)
        case _:
            raise WorldCheckError("Input backup wire rows missing")
    role = text(at(descriptor, ("role",)))
    end = max(ids, default=-1) + 1
    before: Json = {
        "first_free": 0,
        "first_unused": end,
        "items": len(ids),
        "occupied": list(ids),
        "slots": end,
    }
    after: Json = {
        "first_free": 0,
        "first_unused": end,
        "items": 0,
        "occupied": [],
        "slots": end,
    }
    context = {
        "sp": "SinglePlayer",
        "server": "NetworkServer",
        "client": "NetworkClient",
    }[role]
    expected: Json = {
        "before": before,
        "after": before if role == "client" else after,
        "deleted": [] if role == "client" else list(ids),
        "context": context,
    }
    if (
        not exact(at(rust, ("load_receipt",)), expected)
        or at(rust, ("declared_context",)) != role
        or not exact(at(rust, ("exit_requested",)), at(descriptor, ("exit",)))
    ):
        raise WorldCheckError("Loaded backup lifetime/context receipt differs")
