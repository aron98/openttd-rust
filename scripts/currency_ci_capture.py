from __future__ import annotations

from pathlib import Path

from scripts.currency_ci_compare import controls
from scripts.currency_ci_evidence import (
    case,
    coverage,
    fingerprint,
    load_case,
    stable_context,
)
from scripts.currency_ci_roster import API, COMPILER_INPUTS, GUARDS, LOAD
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import sequence
from scripts.language_ci_bindings import manifest_digest, normalized
from scripts.language_ci_compare import compare, mapping
from scripts.world_check_support import Json, WorldCheckError, at, read_json


def input_binding(root: Path, directory: Path, oracle: Path) -> dict[str, Json]:
    inputs = [
        path
        for path in directory.rglob("*")
        if path.is_file()
        and (
            (path.parent == directory and path.suffix in {".grf", ".cfg", ".cmake"})
            or path.parent == directory / "pack"
        )
    ]
    return {
        "paths": [
            str(path.relative_to(directory))
            for path in sorted(directory.rglob("*"))
            if path.is_file()
        ],
        "argv": normalized(
            read_json(directory / "argv.json"), (root, directory, oracle)
        ),
        "manifest_sha256": manifest_digest(
            read_json(directory / "manifest.json"), (root, directory, oracle)
        ),
        "inputs": {str(path.relative_to(directory)): digest(path) for path in inputs},
    }


def observed_case(root: Path, directory: Path, oracle: Path, *, api: bool) -> Json:
    item = input_binding(root, directory, oracle)
    native = read_json(directory / "native/currency.json")
    item["native_observables_sha256"] = fingerprint(
        stable_context(native), (root, directory, oracle.parent)
    )
    if api:
        rust = read_json(directory / "rust.json")
        compare(at(native, ("results",)), rust)
        item["coverage"] = {
            "operations": len(sequence(rust)),
            "controls": controls(rust, read_json(directory / "controls.json")),
        }
    else:
        item["coverage"] = {
            "loads": [
                load_case(directory, load, index)
                for index, load in enumerate(sequence(at(native, ("loads",))))
            ]
        }
    _ = case(root, directory, oracle, item, api=api)
    return item


def guard_diagnostic(name: str) -> str:
    diagnostics = {
        "menu": "",
        "unarmed": "",
        "replay": "refuses replay or case subset",
        "subset": "refuses replay or case subset",
        "reused": "run directory already exists",
        "non-save": "requires an explicit saved-game load",
        "stale": "Stale native replay binary/source",
        "duplicate": "currency output exists",
        "pending": "currency API must finalize queued destinations",
        "stale-read": "stale custom currency string query",
        "mixed": "invalid or duplicate currency fixture",
        "reload-index": "currency reload duplicate or invalid config",
        "reload-duplicate": "currency reload duplicate or invalid config",
    }
    return diagnostics[name]


def source_names(root: Path) -> tuple[str, ...]:
    paths = [
        root / name
        for name in (
            "Cargo.toml",
            "Cargo.lock",
            "upstream.toml",
            "rust-toolchain.toml",
            "scripts/reference.cfg",
            "fixtures/replay/clear-v362.sav",
            *COMPILER_INPUTS,
        )
    ]
    paths.extend(
        path
        for folder in ("crates", "reference", "scripts")
        for path in (root / folder).rglob("*")
        if path.is_file()
        and (
            path.suffix in {".rs", ".toml", ".py", ".sh", ".cmake", ".hpp", ".patch"}
            or (folder == "crates" and path.suffix == ".json")
        )
    )
    if not paths:
        raise WorldCheckError("Missing currency source inventory")
    return tuple(sorted({str(path.relative_to(root)) for path in paths}))


def capture(root: Path, api: Path, load: Path, guards: Path, oracle: Path) -> Json:
    result: dict[str, Json] = {"verified_native_corpus": True}
    invocations: list[Json] = []
    for group, directory, names in (("api", api, API), ("load", load, LOAD)):
        summary = read_json(directory / "summary.json")
        compare(
            [at(row, ("case",)) for row in sequence(summary)], [str(v) for v in names]
        )
        result[group] = {
            "summary": summary,
            "cases": {
                name: observed_case(root, directory / name, oracle, api=group == "api")
                for name in names
            },
        }
        invocations.extend(
            f"{group}/{path.relative_to(directory)}"
            for path in directory.rglob("invocation.txt")
        )
    declared_guards: dict[str, Json] = {}
    for name in GUARDS:
        directory = guards / name
        item = input_binding(root, directory, oracle)
        item["status"] = read_json(directory / "status.json")
        item["diagnostic"] = guard_diagnostic(name)
        if name == "stale":
            paths = [
                str(p)
                for p in sequence(item["paths"])
                if not str(p).startswith("stale-source/")
            ]
            paths.extend(
                f"stale-source/{folder}/{path.name}"
                for folder in ("scripts", "reference")
                for path in (root / folder).iterdir()
                if path.is_file()
            )
            own = "stale-source/scripts/currency-ci-layout.json"
            if own not in paths:
                paths.append(own)
            item["paths"] = [str(v) for v in sorted(paths)]
        declared_guards[name] = item
    result["guards"] = declared_guards
    invocations.extend(
        f"guards/{path.relative_to(guards)}" for path in guards.rglob("invocation.txt")
    )
    result["native_invocations"] = sorted(invocations, key=str)
    result["focused"] = [
        "content::grf::load_currency_session_tests::currency_repeated_assignments_obey_cumulative_trace_budget",
        "content::grf::load_currency_session_tests::currency_adjacent_properties_remain_unsupported",
    ]
    result["sources"] = {name: digest(root / name) for name in source_names(root)}
    result["totals"] = {
        "guards": 13,
        **{
            group: coverage(
                [
                    at(item, ("coverage",))
                    for item in mapping(at(result, (group, "cases"))).values()
                ],
                api=group == "api",
            )
            for group in ("api", "load")
        },
    }
    return result
