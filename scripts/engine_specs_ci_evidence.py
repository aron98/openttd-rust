from __future__ import annotations

import hashlib
import json
from pathlib import Path

from scripts.context_ci_support import process, sources, verify_identity
from scripts.currency_ci_baseline import bind_baseline
from scripts.currency_ci_bindings import cargo_binding, invocations
from scripts.engine_specs_ci_roster import CASES, COMPILER_INPUT, EVENT_COUNTS, SELECTOR
from scripts.gameplay_foundations import log_name
from scripts.grf_control_evidence import sequence, text
from scripts.language_ci_compare import compare, mapping
from scripts.world_check_support import Json, WorldCheckError, at, read_json


def fingerprint(value: Json) -> str:
    encoded = json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=False
    )
    return hashlib.sha256(encoded.encode()).hexdigest()


def projection(native: Json, control: Json, checkpoint: str) -> Json:
    rows: list[Json] = []
    for event in sequence(at(native, ("events",))):
        if at(event, ("phase",)) == checkpoint:
            state = dict(mapping(at(event, ("state",))))
            if set(state) != {
                "owners",
                "mappings",
                "temporary",
                "grfid_overrides",
                "pool_capacity",
                "dynamic_engines",
                "context",
            }:
                raise WorldCheckError("Engine state schema changed")
            del state["context"]
            rows.append(
                {
                    "detail": at(event, ("detail",))
                    if checkpoint == "api-command"
                    else None,
                    "state": state,
                }
            )
    return {
        "mode": "api" if checkpoint == "api-command" else "load",
        "results": rows,
        "files": sequence(at(control, ("files",)))[2:],
    }


def case(
    root: Path, directory: Path, oracle: Path, item: Json, event_count: int
) -> None:
    native = read_json(directory / "native/specs.json")
    control = read_json(directory / "native/control.json")
    manifest = read_json(directory / "manifest.json")
    mode = text(at(manifest, ("engine_specs", "mode")))
    compare(
        at(native, ("mode",)),
        "original-engine-spec-api-v1"
        if mode == "api"
        else "actual-loader-engine-spec-v1",
    )
    for document in (native, control):
        compare(at(document, ("arms",)), 1)
        compare(at(document, ("consumptions",)), 1)
    compare(at(native, ("input",)), at(manifest, ("engine_specs",)))
    compare(read_json(directory / "native/native-status.json"), "0")
    bind_baseline(directory, oracle, native)
    compare(len(sequence(at(native, ("events",)))), event_count)
    for event in sequence(at(native, ("events",))):
        for field in ("random", "interactive_random"):
            compare(
                at(event, ("state", "context", field)), at(control, ("before", field))
            )
    for field in ("random", "interactive_random"):
        for phase in ("prepared", "after"):
            compare(at(control, (phase, field)), at(control, ("before", field)))
    checkpoint = text(at(item, ("checkpoint",)))
    observed = projection(native, control, checkpoint)
    compare(len(sequence(at(observed, ("results",)))), at(item, ("rows",)))
    compare(observed, read_json(directory / "rust.json"))
    compare(fingerprint(observed), at(item, ("fingerprint",)))
    definitions = sequence(at(read_json(root / COMPILER_INPUT), ("cases",)))
    definition = next(
        (row for row in definitions if at(row, ("name",)) == directory.name), None
    )
    if definition is None:
        raise WorldCheckError("Unknown engine corpus case")
    compare(mode, at(definition, ("mode",)))
    compare(
        at(manifest, ("engine_specs", "dynamic_engines")),
        at(definition, ("dynamic_engines",)),
    )
    if mode == "api":
        compare(
            at(manifest, ("engine_specs", "commands")), at(definition, ("commands",))
        )
    configured = sequence(at(manifest, ("files",)))
    declared = sequence(at(definition, ("files",)))
    compare(len(configured), len(declared))
    for index, (actual, expected) in enumerate(zip(configured, declared, strict=True)):
        path = directory / f"{index}.grf"
        compare(at(actual, ("path",)), str(path))
        compare(list(path.read_bytes()), at(expected, ("bytes",)))
        compare(at(actual, ("grfid",)), at(expected, ("grfid",)))
        compare(
            {
                key: value
                for key, value in mapping(actual).items()
                if key not in {"path", "grfid"}
            },
            {
                "metadata_version": 0,
                "parameters": [],
                "static": False,
                "init_only": False,
                "system": False,
            },
        )


def invocation_paths(output: Path) -> list[Json]:
    return [
        *sorted(
            str(path.relative_to(output))
            for path in output.rglob("invocation.txt")
            if not path.is_relative_to(output / "corruption")
        )
    ]


def validate(root: Path, output: Path, oracle: Path, layout: Json) -> None:
    sources(root, layout)
    verify_identity(root, output)
    cargo_binding(output)
    environment = mapping(read_json(output / "build-environment.json"))
    target = Path(text(at(environment, ("CARGO_TARGET_DIR",))))
    if (
        not target.is_absolute()
        or target.resolve().is_relative_to(root)
        or target.resolve().is_relative_to(output)
    ):
        raise WorldCheckError("Engine Cargo target escaped external allocation")
    compare(
        environment,
        {
            "CARGO_TARGET_DIR": str(target),
            "CARGO_INCREMENTAL": "0",
            "CARGO_PROFILE_DEV_DEBUG": "0",
            "CARGO_PROFILE_TEST_DEBUG": "0",
            "CARGO_BUILD_JOBS": "1",
        },
    )
    binary = Path(
        text(at(read_json(output / "test-binaries.json"), ("ottd_sim", "retained")))
    )
    process(output / "logs" / log_name(SELECTOR), binary, SELECTOR)
    compare(read_json(output / "cases/summary.json"), list(CASES))
    compare(
        sorted(p.name for p in (output / "cases").iterdir() if p.is_dir())
        == sorted(CASES),
        rust=True,
    )
    compare(sorted(mapping(at(layout, ("cases",)))) == sorted(CASES), rust=True)
    for name, count in zip(CASES, EVENT_COUNTS, strict=True):
        compare(
            read_json(output / "logs" / log_name("native-" + name) / "argv.json"),
            [
                "cmake",
                f"-DORACLE={oracle}",
                f"-DRUN_DIR={output / 'cases' / name / 'native'}",
                f"-DCONFIG={root / 'scripts/reference.cfg'}",
                f"-DINPUT={root / 'fixtures/replay/clear-v362.sav'}",
                f"-DMANIFEST={output / 'cases' / name / 'manifest.json'}",
                "-P",
                str(root / "scripts/check-engine-specs-reference.cmake"),
            ],
        )
        receipt = read_json(
            output / "logs" / log_name("native-" + name) / "process.json"
        )
        compare(at(receipt, ("returncode",)), 0)
        case(root, output / "cases" / name, oracle, at(layout, ("cases", name)), count)
    invocations(root, output, oracle, invocation_paths(output))
