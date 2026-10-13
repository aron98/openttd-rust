from __future__ import annotations

from copy import deepcopy
from pathlib import Path

from scripts.context_ci_support import sources, verify_identity
from scripts.language_ci_bindings import bound_inputs
from scripts.language_ci_probes import modified_file, rejected
from scripts.strings_ci_evidence import case, membership
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    read_json,
    replace,
    write_json,
)


def live_probes(root: Path, output: Path, oracle: Path, layout: Json) -> None:
    directory = output / "corruption"
    directory.mkdir()
    target = output / "load/load-after-order"
    expected = at(layout, ("load", "cases", "load-after-order"))
    mutations: list[tuple[str, str, tuple[str | int, ...], Json]] = [
        ("native-final", "native/strings.json", ("final_table", "entries"), []),
        ("rust-final", "rust-final.json", ("table", "entries"), []),
        (
            "native-error",
            "native/strings.json",
            ("translation_errors", 0, "data"),
            [255],
        ),
        ("rust-error", "rust-final.json", ("translation_errors", 0, "message"), 0),
        ("rust-control", "rust-control.json", ("events",), []),
        ("native-events", "native/strings.json", ("events",), []),
        ("control-execution", "controls.json", (0, "rejected"), False),
    ]
    rows: list[Json] = []
    for name, file, pointer, value in mutations:
        source = target / file
        changed = deepcopy(read_json(source))
        if at(changed, pointer) == value:
            raise WorldCheckError("Ineffective string admission probe")
        replace(changed, pointer, value)
        artifact = directory / f"{name}.json"
        write_json(artifact, changed)
        rows.append(
            rejected(
                name,
                lambda source=source, artifact=artifact: modified_file(
                    source, artifact.read_bytes(), lambda: check(target, expected)
                ),
            )
        )
    summary = read_json(output / "load/summary.json")
    write_json(directory / "missing-case.json", [])
    rows.append(rejected("missing-case", lambda: membership([], summary)))
    wrong = deepcopy(layout)
    replace(wrong, ("sources", "Cargo.toml"), "0" * 64)
    write_json(directory / "changed-source.json", wrong)
    rows.append(rejected("source", lambda: sources(root, wrong)))
    pack = target / "pack/input.lng"
    changed_pack = pack.read_bytes() + b"changed"
    _ = (directory / "changed-input.lng").write_bytes(changed_pack)
    rows.append(
        rejected(
            "input",
            lambda: modified_file(
                pack,
                changed_pack,
                lambda: bound_inputs(target, expected, (root, target, oracle)),
            ),
        )
    )
    binaries = output / "test-binaries.json"
    wrong_binary = deepcopy(read_json(binaries))
    replace(wrong_binary, ("ottd_sim", "sha256"), "0" * 64)
    write_json(directory / "changed-binary.json", wrong_binary)
    rows.append(
        rejected(
            "executable",
            lambda: modified_file(
                binaries,
                (directory / "changed-binary.json").read_bytes(),
                lambda: verify_identity(root, output),
            ),
        )
    )
    write_json(directory / "rejections.json", rows)


def check(directory: Path, expected: Json) -> None:
    _ = case(directory, expected, api=False)
