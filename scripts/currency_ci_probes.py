from __future__ import annotations

from copy import deepcopy
from pathlib import Path
from stat import S_IMODE, S_IWUSR

from scripts.context_ci_support import sources, verify_identity
from scripts.currency_ci_bindings import cargo_binding
from scripts.currency_ci_evidence import case, validate_currency
from scripts.grf_control_evidence import sequence
from scripts.language_ci_bindings import bound_inputs
from scripts.language_ci_probes import modified_file, rejected
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
    target = output / "api/ordered-writes"
    item = at(layout, ("api", "cases", "ordered-writes"))
    raw = read_json(target / "native/currency.json")
    last = len(sequence(at(raw, ("results",)))) - 1
    mutations: list[tuple[str, str, tuple[str | int, ...], Json]] = [
        (
            "native-owner",
            "native/currency.json",
            ("results", last, "owners", 0, "name"),
            None,
        ),
        ("rust-owner", "rust.json", (last, "owners", 0, "name"), None),
        ("native-queue", "native/currency.json", ("results", 0, "pending"), []),
        ("unexecuted-control", "controls.json", (0, "rejected"), False),
        (
            "one-sided-context-rng",
            "native/currency.json",
            ("after", "interactive_random"),
            [0, 0],
        ),
    ]
    rows: list[Json] = []
    for name, file, pointer, value in mutations:
        source = target / file
        changed = deepcopy(read_json(source))
        if at(changed, pointer) == value:
            raise WorldCheckError("Ineffective currency admission probe")
        replace(changed, pointer, value)
        artifact = directory / f"{name}.json"
        write_json(artifact, changed)
        rows.append(
            rejected(
                name,
                lambda source=source, artifact=artifact: modified_file(
                    source,
                    artifact.read_bytes(),
                    lambda: check(root, target, oracle, item),
                ),
            )
        )
    rows.append(paired((root, target, oracle), item, directory, last))
    rows.append(reordered(root, output, oracle, layout, directory))
    summary = read_json(output / "load/summary.json")
    missing: Json = list(sequence(summary)[:-1])
    write_json(directory / "missing-case.json", missing)
    rows.append(
        rejected(
            "missing-case",
            lambda: modified_file(
                output / "load/summary.json",
                (directory / "missing-case.json").read_bytes(),
                lambda: validate_currency(root, output, oracle, layout),
            ),
        )
    )
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
                lambda: bound_inputs(target, item, (root, target, oracle)),
            ),
        )
    )
    selected_binary(output, directory, root, rows)
    wrong_selection = deepcopy(read_json(output / "test-binaries.json"))
    replace(wrong_selection, ("ottd_sim", "original"), str(output / "bin/ottd_sim"))
    write_json(directory / "wrong-cargo-selection.json", wrong_selection)
    rows.append(
        rejected(
            "wrong-cargo-selection",
            lambda: modified_file(
                output / "test-binaries.json",
                (directory / "wrong-cargo-selection.json").read_bytes(),
                lambda: cargo_binding(output),
            ),
        )
    )
    write_json(directory / "rejections.json", rows)


def paired(
    roots: tuple[Path, Path, Path],
    item: Json,
    directory: Path,
    last: int,
) -> Json:
    root, target, oracle = roots
    native = deepcopy(read_json(target / "native/currency.json"))
    rust = deepcopy(read_json(target / "rust.json"))
    replace(native, ("results", last, "owners", 0, "name"), None)
    replace(rust, (last, "owners", 0, "name"), None)
    write_json(directory / "paired-native.json", native)
    write_json(directory / "paired-rust.json", rust)
    return rejected(
        "paired-owner",
        lambda: modified_file(
            target / "native/currency.json",
            (directory / "paired-native.json").read_bytes(),
            lambda: modified_file(
                target / "rust.json",
                (directory / "paired-rust.json").read_bytes(),
                lambda: check(root, target, oracle, item),
            ),
        ),
    )


def reordered(
    root: Path,
    output: Path,
    oracle: Path,
    layout: Json,
    directory: Path,
) -> Json:
    target = output / "load/deferred-and-overwrite"
    source = target / "load-0/rust.json"
    original = read_json(source)
    changed: Json = list(reversed(sequence(original)))
    if changed == original:
        raise WorldCheckError("Ineffective currency reorder")
    write_json(directory / "reordered-state.json", changed)
    item = at(layout, ("load", "cases", "deferred-and-overwrite"))
    return rejected(
        "reordered-state",
        lambda: modified_file(
            source,
            (directory / "reordered-state.json").read_bytes(),
            lambda: check(root, target, oracle, item, api=False),
        ),
    )


def selected_binary(
    output: Path, directory: Path, root: Path, rows: list[Json]
) -> None:
    binary = output / "bin/ottd_sim"
    original = binary.read_bytes()
    changed = b"X" + original[1:]
    if changed == original:
        raise WorldCheckError("Ineffective executable mutation")
    artifact = directory / "altered-selected-executable"
    _ = artifact.write_bytes(changed)
    mode = S_IMODE(binary.stat().st_mode)
    try:
        binary.chmod(mode | S_IWUSR)
        rows.append(
            rejected(
                "retained-executable",
                lambda: modified_file(
                    binary,
                    changed,
                    lambda: verify_identity(root, output),
                ),
            )
        )
    finally:
        binary.chmod(mode)


def check(
    root: Path, target: Path, oracle: Path, item: Json, *, api: bool = True
) -> None:
    _ = case(root, target, oracle, item, api=api)
