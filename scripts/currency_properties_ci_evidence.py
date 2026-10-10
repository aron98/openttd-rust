from __future__ import annotations

from pathlib import Path

from scripts.currency_ci_bindings import cargo_binding, context_binding, invocations
from scripts.currency_ci_capture import observed_case
from scripts.currency_ci_evidence import case, coverage
from scripts.currency_properties_ci_roster import API, GUARDS, LOAD
from scripts.depot_build_archive import bounded_paths
from scripts.grf_control_evidence import sequence, text
from scripts.language_ci_bindings import bound_inputs, normalized
from scripts.language_ci_compare import compare, mapping, number
from scripts.strings_ci_bindings import positive
from scripts.world_check_support import Json, WorldCheckError, at, read_json


def byte_owners(owners: Json) -> None:
    compare(len(sequence(owners)), 46)
    for owner in sequence(owners):
        for field in ("prefix", "suffix"):
            for byte in sequence(at(owner, (field,))):
                if number(byte) > 255:
                    raise WorldCheckError("Out-of-range currency symbol byte")


def property_case(
    root: Path, directory: Path, oracle: Path, item: Json, *, api: bool
) -> Json:
    native = read_json(directory / "native/currency.json")
    compare(at(native, ("currency_owner_encoding",)), "bytes-v1")
    if api:
        byte_owners(at(native, ("initial", "owners")))
        for row in sequence(at(native, ("results",))):
            byte_owners(at(row, ("owners",)))
    else:
        for load in sequence(at(native, ("loads",))):
            byte_owners(at(load, ("owners",)))
            for event in sequence(at(load, ("events",))):
                byte_owners(at(event, ("owners",)))
    result = case(root, directory, oracle, item, api=api)
    positive(root, directory, oracle, item)
    context_binding(directory, api=api)
    return result


def guard(root: Path, directory: Path, oracle: Path, item: Json) -> None:
    _ = bounded_paths(directory, {text(v) for v in sequence(at(item, ("paths",)))})
    bound_inputs(directory, item, (root, directory, oracle))
    compare(
        normalized(read_json(directory / "argv.json"), (root, directory, oracle)),
        at(item, ("argv",)),
    )
    status = read_json(directory / "status.json")
    compare(status, at(item, ("status",)))
    if number(status) == 0:
        raise WorldCheckError("Currency property guard succeeded")
    errors = (directory / "stderr.log").read_text()
    native_error = directory / "native/stderr.log"
    if native_error.exists():
        errors += native_error.read_text()
    if text(at(item, ("diagnostic",))) not in errors:
        raise WorldCheckError("Missing property guard diagnostic")
    if (directory / "native/currency.json").exists():
        raise WorldCheckError("Property guard emitted observation")


def validate(root: Path, output: Path, oracle: Path, layout: Json) -> None:
    if at(layout, ("verified_native_corpus",)) is not True:
        raise WorldCheckError("Property corpus awaits actual native capture")
    totals: dict[str, Json] = {}
    for group, names in (("api", API), ("load", LOAD)):
        compare(
            [
                text(at(row, ("case",)))
                for row in sequence(read_json(output / group / "summary.json"))
            ],
            list(names),
        )
        compare(
            read_json(output / group / "summary.json"), at(layout, (group, "summary"))
        )
        cases = mapping(at(layout, (group, "cases")))
        compare(sorted(cases) == sorted(names), rust=True)
        compare(
            sorted(p.name for p in (output / group).iterdir() if p.is_dir())
            == sorted(names),
            rust=True,
        )
        totals[group] = coverage(
            [
                property_case(
                    root, output / group / name, oracle, item, api=group == "api"
                )
                for name, item in cases.items()
            ],
            api=group == "api",
        )
    guards = mapping(at(layout, ("guards",)))
    compare(sorted(guards) == sorted(GUARDS), rust=True)
    compare(
        sorted(p.name for p in (output / "guards").iterdir() if p.is_dir())
        == sorted(guards),
        rust=True,
    )
    for name, item in guards.items():
        guard(root, output / "guards" / name, oracle, item)
    totals["guards"] = len(guards)
    compare(totals, at(layout, ("totals",)))
    cargo_binding(output)
    invocations(root, output, oracle, at(layout, ("native_invocations",)))


def capture_cases(root: Path, output: Path, oracle: Path) -> Json:
    result: dict[str, Json] = {}
    for group, names in (("api", API), ("load", LOAD)):
        summary = read_json(output / group / "summary.json")
        compare([text(at(row, ("case",))) for row in sequence(summary)], list(names))
        captured: dict[str, Json] = {}
        for name in names:
            directory = output / group / name
            item = observed_case(root, directory, oracle, api=group == "api")
            _ = property_case(root, directory, oracle, item, api=group == "api")
            captured[name] = item
        result[group] = {"summary": summary, "cases": captured}
    return result
