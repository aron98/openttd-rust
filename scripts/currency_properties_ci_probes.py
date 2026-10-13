from __future__ import annotations

from copy import deepcopy
from pathlib import Path

from scripts.context_ci_support import sources
from scripts.currency_ci_bindings import cargo_binding
from scripts.currency_ci_probes import selected_binary
from scripts.currency_properties_ci_evidence import property_case, validate
from scripts.currency_properties_ci_roster import COMPILER_INPUT, PROBES
from scripts.grf_control_evidence import sequence
from scripts.language_ci_compare import compare
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
    target = output / "api/symbols-utf8"
    item = at(layout, ("api", "cases", "symbols-utf8"))
    rows: list[Json] = []

    def check_case() -> None:
        _ = property_case(root, target, oracle, item, api=True)

    check_case()
    sources(root, layout)
    cargo_binding(output)
    mutations: list[tuple[str, str, tuple[str | int, ...], Json]] = [
        (
            "native-symbol",
            "native/currency.json",
            ("results", 0, "owners", 0, "prefix"),
            [],
        ),
        ("rust-symbol", "rust.json", (0, "owners", 0, "prefix"), []),
        ("false-control", "controls.json", (0, "rejected"), False),
        (
            "wrong-encoding",
            "native/currency.json",
            ("currency_owner_encoding",),
            "text-v1",
        ),
        (
            "one-sided-rng",
            "native/currency.json",
            ("after", "interactive_random"),
            [0, 0],
        ),
    ]
    for name, filename, pointer, value in mutations:
        changed = deepcopy(read_json(target / filename))
        if at(changed, pointer) == value:
            raise WorldCheckError("Ineffective property evidence mutation")
        replace(changed, pointer, value)
        artifact = directory / f"{name}.json"
        write_json(artifact, changed)
        rows.append(
            rejected(
                name,
                lambda filename=filename, artifact=artifact: modified_file(
                    target / filename,
                    artifact.read_bytes(),
                    check_case,
                ),
            )
        )
    native, rust = (
        deepcopy(read_json(target / "native/currency.json")),
        deepcopy(read_json(target / "rust.json")),
    )
    replace(native, ("results", 0, "owners", 0, "prefix"), [239, 191, 189])
    replace(rust, (0, "owners", 0, "prefix"), [239, 191, 189])
    write_json(directory / "paired-native.json", native)
    write_json(directory / "paired-rust.json", rust)
    rows.append(
        rejected(
            "paired-lossy-symbol",
            lambda: modified_file(
                target / "native/currency.json",
                (directory / "paired-native.json").read_bytes(),
                lambda: modified_file(
                    target / "rust.json",
                    (directory / "paired-rust.json").read_bytes(),
                    check_case,
                ),
            ),
        )
    )
    summary = sequence(read_json(output / "api/summary.json"))
    write_json(directory / "missing-case.json", summary[:-1])
    rows.append(
        rejected(
            "missing-api-case",
            lambda: modified_file(
                output / "api/summary.json",
                (directory / "missing-case.json").read_bytes(),
                lambda: validate(root, output, oracle, layout),
            ),
        )
    )
    loader = output / "load/deferred-and-overwrite/load-0/rust.json"
    ordered = sequence(read_json(loader))
    changed_order: Json = list(reversed(ordered))
    if ordered == changed_order:
        raise WorldCheckError("Ineffective property order mutation")
    write_json(directory / "reordered.json", changed_order)
    rows.append(
        rejected(
            "reordered-loader",
            lambda: modified_file(
                loader,
                (directory / "reordered.json").read_bytes(),
                lambda: validate(root, output, oracle, layout),
            ),
        )
    )
    wrong_source = deepcopy(layout)
    replace(wrong_source, ("sources", "Cargo.toml"), "0" * 64)
    write_json(directory / "wrong-source.json", wrong_source)
    rows.append(rejected("source", lambda: sources(root, wrong_source)))
    compiled = root / COMPILER_INPUT
    altered = compiled.read_bytes() + b"\n"
    _ = (directory / "changed-compiled-input.json").write_bytes(altered)
    rows.append(
        rejected(
            "compiled-input",
            lambda: modified_file(compiled, altered, lambda: sources(root, layout)),
        )
    )
    selected_binary(output, directory, root, rows)
    wrong = deepcopy(read_json(output / "test-binaries.json"))
    replace(wrong, ("ottd_sim", "original"), str(output / "bin/ottd_sim"))
    write_json(directory / "wrong-cargo-selection.json", wrong)
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
    compare([at(row, ("name",)) for row in rows], list(PROBES))
    check_case()
    write_json(directory / "rejections.json", rows)
