from __future__ import annotations

from pathlib import Path

from scripts.grf_control_evidence import sequence, text
from scripts.language_ci_bindings import hash_rows
from scripts.language_ci_compare import compare, mapping
from scripts.world_check_support import Json, WorldCheckError, read_json


def canonical_transport(value: Json, oracle_directory: Path) -> Json:
    match value:
        case {
            "mode": (
                "separate-process-original-currency-api"
                | "actual-loader-currency-owners"
            ),
            "baseline_sources": list() as paths,
        }:
            builtin = oracle_directory / "baseset/openttd.grf"
            if paths and paths[0] == str(builtin):
                return {
                    **value,
                    "baseline_sources": [
                        str(builtin.with_name("OPENTTD.GRF")),
                        *paths[1:],
                    ],
                }
        case _:
            pass
    return value


def bind_baseline(directory: Path, oracle: Path, native: Json) -> None:
    raw = mapping(native).get("baseline_sources")
    compare(
        raw,
        mapping(read_json(directory / "native/control.json")).get("baseline_sources"),
    )
    paths = [Path(text(value)) for value in sequence(raw)]
    baseset = oracle.parent / "baseset"
    builtins = {baseset / "openttd.grf", baseset / "OPENTTD.GRF"}
    if (
        len(paths) != 2
        or paths[0] not in builtins
        or any(not path.resolve().is_relative_to(baseset.resolve()) for path in paths)
    ):
        raise WorldCheckError("Currency baseline sources escaped ordered oracle inputs")
    hash_rows(directory / "native/baseline-inputs.sha256", paths)
