# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through scripts/check-order-state.py.
from __future__ import annotations

from pathlib import Path

from scripts.gameplay_foundations import digest as sha
from scripts.gameplay_foundations import require_test
from scripts.order_state_evidence import validate_pair
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    read_json,
    run,
    write_json,
)


def object_at(value: Json, path: tuple[str | int, ...] = ()) -> dict[str, Json]:
    match at(value, path):
        case dict() as fields:
            return fields
        case _:
            raise WorldCheckError("Expected native order observation object")


def shared_projection(value: Json, *, native: bool) -> Json:
    rows: list[Json] = []
    match at(value, ("actions",)):
        case list() as actions:
            for action in actions:
                fields = object_at(action)
                result = fields["result"]
                if native and at(fields["input"], ("op",)) == "save":
                    saved = object_at(result)
                    if set(saved) != {"path", "before", "after", "role"}:
                        raise WorldCheckError("Native save result schema differs")
                    result = {key: saved[key] for key in ("path", "before", "after")}
                rows.append(
                    {
                        "index": fields["index"],
                        "input": fields["input"],
                        "before": fields["before"],
                        "result": result,
                        "after": fields["after"],
                    }
                )
        case _:
            raise WorldCheckError("Missing complete order action observations")
    return {
        "schema_version": at(value, ("schema_version",)),
        "case": at(value, ("case",)),
        "initial": at(value, ("initial",)),
        "actions": rows,
        "final": at(value, ("final",)),
    }


def compare_case(manifest: Json, case: Path, output: Path) -> None:
    output.mkdir(parents=True, exist_ok=False)
    inputs = read_json(case / "inputs.json")
    source = Path(str(at(inputs, ("save",))))
    descriptor = case / "actions.json"
    runner = Path(str(at(manifest, ("runner", "executable"))))
    cli = Path(str(at(manifest, ("cli", "executable"))))
    if sha(runner) != at(manifest, ("runner", "sha256")) or sha(cli) != at(
        manifest, ("cli", "sha256")
    ):
        raise WorldCheckError("Order Rust executable identity changed")
    selector = str(at(manifest, ("case_test",)))
    invocation = [
        "env",
        f"ORDER_CASE_INPUT={source}",
        f"ORDER_CASE_ACTIONS={descriptor}",
        f"ORDER_CASE_OUTPUT={output / 'rust'}",
        str(runner),
        "--exact",
        selector,
        "--ignored",
        "--nocapture",
    ]
    _ = run(
        [str(cli), "world", str(source), "--view", "saved"], output / "input-export"
    )
    result = run(invocation, output / "rust-command")
    require_test(result.stdout, selector)
    if sha(source) != at(inputs, ("save_sha256",)) or sha(descriptor) != at(
        inputs, ("descriptor_sha256",)
    ):
        raise WorldCheckError("Order input identity changed")
    native = read_json(case / "original/native/results.json")
    rust = read_json(output / "rust/results.json")
    write_json(output / "native-state.json", shared_projection(native, native=True))
    write_json(output / "rust-state.json", shared_projection(rust, native=False))
    _ = run(
        [
            str(cli),
            "compare",
            str(output / "native-state.json"),
            str(output / "rust-state.json"),
        ],
        output / "live-compare",
    )
    for save in sorted((case / "original/native").glob("*.sav")):
        other = output / "rust" / save.name
        for label, path in (("native", save), ("rust", other)):
            exported = run(
                [str(cli), "world", str(path), "--view", "saved"],
                output / f"{save.stem}-{label}-export",
            )
            _ = (output / f"{save.stem}-{label}.json").write_text(exported.stdout)
        _ = run(
            [
                str(cli),
                "compare",
                str(output / f"{save.stem}-native.json"),
                str(output / f"{save.stem}-rust.json"),
            ],
            output / f"{save.stem}-saved-compare",
        )
    _ = run(
        [
            str(cli),
            "compare",
            str(case / "original/saved-world.json"),
            str(output / "after-rust.json"),
        ],
        output / "native-serializer-compare",
    )

    validate_pair(case, output, read_json(descriptor), manifest)
    write_json(
        output / "comparison.json",
        {
            "passed": True,
            "case": case.name,
            "source_sha256": sha(source),
            "descriptor_sha256": sha(descriptor),
            "runner_sha256": sha(runner),
            "cli_sha256": sha(cli),
        },
    )
