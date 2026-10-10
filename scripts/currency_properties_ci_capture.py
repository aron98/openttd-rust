from __future__ import annotations

from pathlib import Path

from scripts.context_ci_support import process, sources, verify_identity
from scripts.currency_ci_bindings import cargo_binding, invocations
from scripts.currency_ci_capture import input_binding
from scripts.currency_ci_evidence import coverage
from scripts.currency_properties_ci_evidence import capture_cases, guard
from scripts.currency_properties_ci_roster import (
    COMPILER_INPUT,
    FOCUSED,
    GUARD_INPUT,
    GUARDS,
    SELECTORS,
)
from scripts.gameplay_foundations import digest, log_name
from scripts.grf_control_evidence import sequence, text
from scripts.language_ci_compare import compare, mapping
from scripts.world_check_support import Json, at, read_json


def capture(root: Path, output: Path, oracle: Path) -> Json:
    cargo_binding(output)
    verify_identity(root, output)
    selected = Path(
        text(at(read_json(output / "test-binaries.json"), ("ottd_sim", "retained")))
    )
    for selector in SELECTORS.values():
        process(output / "logs" / log_name(selector), selected, selector)
    result = mapping(capture_cases(root, output, oracle))
    definitions = sequence(at(read_json(root / GUARD_INPUT), ("mutations",)))
    compare([text(at(item, ("case",))) for item in definitions], list(GUARDS))
    guards: dict[str, Json] = {}
    for definition in definitions:
        name = text(at(definition, ("case",)))
        directory = output / "guards" / name
        item = input_binding(root, directory, oracle)
        item["diagnostic"] = at(definition, ("expect",))
        item["status"] = read_json(directory / "status.json")
        guard(root, directory, oracle, item)
        guards[name] = item
    result["guards"] = guards
    result["focused"] = list(FOCUSED)
    ordered_paths = sorted(
        str(p.relative_to(output)) for p in output.rglob("invocation.txt")
    )
    invocation_paths: list[Json] = list(ordered_paths)
    result["native_invocations"] = invocation_paths
    hashes = {
        name: sha
        for name, sha in mapping(
            at(read_json(output / "provenance.json"), ("source_hashes",))
        ).items()
        if name != "compatibility/contract.json"
        and (
            not name.startswith("scripts/")
            or Path(name).suffix != ".json"
            or name == GUARD_INPUT
        )
    }
    for path in (COMPILER_INPUT, GUARD_INPUT):
        hashes[path] = digest(root / path)
    result["sources"] = hashes
    sources(root, result)
    invocations(root, output, oracle, invocation_paths)
    result["totals"] = {
        "guards": len(GUARDS),
        **{
            group: coverage(
                [
                    at(row, ("coverage",))
                    for row in mapping(at(result, (group, "cases"))).values()
                ],
                api=group == "api",
            )
            for group in ("api", "load")
        },
    }
    result["verified_native_corpus"] = True
    return result
