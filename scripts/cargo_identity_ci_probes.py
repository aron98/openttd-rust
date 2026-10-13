from __future__ import annotations

from copy import deepcopy

from scripts.cargo_identity_ci_evidence import validate
from scripts.cargo_identity_ci_projection import indices
from scripts.cargo_identity_ci_roster import SELECTOR
from scripts.gameplay_foundations import log_name
from scripts.grf_control_evidence import sequence
from scripts.grf_control_run import ControlRun
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


def live_probes(job: ControlRun, layout: Json) -> None:
    directory = job.output / "corruption"
    directory.mkdir()
    target = job.output / "cases/api-v8-small-chain"
    original = read_json(target / "native/cargo.json")
    included, _ = indices(original, target.name)
    first, last = included[0], included[-1]
    rows: list[Json] = []

    def check() -> None:
        validate(job.root, job.output, job.oracle, layout)

    mutations: list[tuple[str, str, tuple[str | int, ...], Json]] = [
        ("cargo-owner", "rust.json", (0, "state", "cargo", 12, "label"), 1),
        (
            "translation",
            "rust.json",
            (len(included) - 1, "state", "files", 0, "cargo_list"),
            [],
        ),
        (
            "temporary-file",
            "rust.json",
            (
                len(included) - 1,
                "state",
                "engines",
                "temporary",
                256,
                "defaultcargo_grfid",
            ),
            None,
        ),
        (
            "reader",
            "native/cargo.json",
            ("events", included[1], "detail", "remaining"),
            999,
        ),
        (
            "baseline-path",
            "native/cargo.json",
            ("baseline_sources", 0),
            "/unbound/openttd.grf",
        ),
        (
            "inherited-mask",
            "native/cargo.json",
            ("events", first, "state", "standard_cargo_mask"),
            0,
        ),
    ]
    for name, filename, pointer, value in mutations:
        changed = deepcopy(read_json(target / filename))
        if at(changed, pointer) == value:
            raise WorldCheckError("Cargo admission mutation is ineffective")
        replace(changed, pointer, value)
        path = directory / (name + ".json")
        write_json(path, changed)
        check()
        rows.append(
            rejected(
                name,
                lambda filename=filename, path=path: modified_file(
                    target / filename, path.read_bytes(), check
                ),
            )
        )
        check()
    paired: list[tuple[str, tuple[str | int, ...], Json]] = [
        ("paired-label", ("cargo", 12, "label"), 1),
        ("paired-translation", ("files", 0, "cargo_list"), []),
        ("paired-road", ("engines", "owners", 256, "info", "cargo_type"), 0),
    ]
    for name, pointer, value in paired:
        native, rust = deepcopy(original), deepcopy(read_json(target / "rust.json"))
        native_pointer = ("events", last, "state", *pointer)
        rust_pointer = (len(included) - 1, "state", *pointer)
        if at(native, native_pointer) == value:
            raise WorldCheckError("Cargo paired mutation is ineffective")
        replace(native, native_pointer, value)
        replace(rust, rust_pointer, value)
        native_path, rust_path = (
            directory / (name + "-native.json"),
            directory / (name + "-rust.json"),
        )
        write_json(native_path, native)
        write_json(rust_path, rust)
        check()
        rows.append(
            rejected(
                name,
                lambda native_path=native_path, rust_path=rust_path: modified_file(
                    target / "native/cargo.json",
                    native_path.read_bytes(),
                    lambda: modified_file(
                        target / "rust.json", rust_path.read_bytes(), check
                    ),
                ),
            )
        )
        check()
    argv = job.output / "logs/build/argv.json"
    selector = job.output / "logs" / log_name(SELECTOR) / "argv.json"
    documents = (
        (
            "cargo-unlocked",
            argv,
            [item for item in sequence(read_json(argv)) if item != "--locked"],
        ),
        (
            "selector",
            selector,
            [
                "nonexistent_cargo_test" if item == SELECTOR else item
                for item in sequence(read_json(selector))
            ],
        ),
        (
            "missing-case",
            job.output / "cases/summary.json",
            list(sequence(read_json(job.output / "cases/summary.json")))[:-1],
        ),
    )
    for name, destination, value in documents:
        path = directory / (name + ".json")
        write_json(path, value)
        check()
        rows.append(
            rejected(
                name,
                lambda destination=destination, path=path: modified_file(
                    destination, path.read_bytes(), check
                ),
            )
        )
        check()
    source = job.root / "reference/cargo_identity.hpp"
    check()
    rows.append(
        rejected(
            "source", lambda: modified_file(source, source.read_bytes() + b"\n", check)
        )
    )
    check()
    compare(len(rows), 13)
    write_json(directory / "summary.json", rows)
