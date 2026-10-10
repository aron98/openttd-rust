from __future__ import annotations

from pathlib import Path

from scripts.context_ci_support import invocation_fields
from scripts.currency_ci_roster import COMPILER_INPUTS, GUARDS
from scripts.depot_build_archive import bounded_paths
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import sequence, text
from scripts.language_ci_bindings import bound_inputs, normalized
from scripts.language_ci_compare import compare, mapping
from scripts.order_state_evidence import command
from scripts.script_vm_provenance import select_executable
from scripts.strings_ci_bindings import positive
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


def compiler_inputs(root: Path, output: Path) -> None:
    path = output / "provenance.json"
    provenance = read_json(path)
    hashes = mapping(at(provenance, ("source_hashes",)))
    for name in COMPILER_INPUTS:
        hashes[name] = digest(root / name)
    write_json(path, provenance)


def guard(root: Path, directory: Path, oracle: Path, item: Json) -> None:
    _ = bounded_paths(directory, {text(v) for v in sequence(at(item, ("paths",)))})
    bound_inputs(directory, item, (root, directory, oracle))
    compare(
        normalized(read_json(directory / "argv.json"), (root, directory, oracle)),
        at(item, ("argv",)),
    )
    compare(read_json(directory / "status.json"), at(item, ("status",)))
    errors = (directory / "stderr.log").read_text()
    native = directory / "native/stderr.log"
    if native.exists():
        errors += native.read_text()
    if text(at(item, ("diagnostic",))) not in errors:
        raise WorldCheckError("Missing actual currency guard diagnostic")
    observation = directory / "currency.json"
    if directory.name == "duplicate":
        compare(observation.read_text(), "sentinel\n")
    elif observation.exists():
        raise WorldCheckError("Currency guard emitted an observation")
    if (directory / "native/currency.json").exists():
        raise WorldCheckError("Currency guard emitted runner observation")
    if directory.name == "stale":
        stale_sources(root, directory)


def stale_sources(root: Path, directory: Path) -> None:
    for folder in ("reference", "scripts"):
        for source in (directory / "stale-source" / folder).iterdir():
            expected = (root / folder / source.name).read_bytes()
            if folder == "reference" and source.name == "grf_currency.hpp":
                expected += b"\n"
            if source.read_bytes() != expected:
                raise WorldCheckError("Unexpected stale currency fixture source")


def invocations(root: Path, output: Path, oracle: Path, required: Json) -> None:
    rows: dict[str, Json] = {}
    for path in sorted(output.rglob("invocation.txt")):
        if path.is_relative_to(output / "corruption"):
            continue
        fields = invocation_fields(root, path)
        if Path(fields["ORACLE"]).resolve() != oracle:
            raise WorldCheckError("Currency native executable path differs")
        compare(fields["ORACLE_SHA256"], digest(oracle))
        for key in ("INPUT", "MANIFEST", "REPLAY"):
            if key in fields:
                compare(fields[f"{key}_SHA256"], digest(Path(fields[key])))
        if "CONFIG_SHA256" in fields:
            config = path.parent.parent / "config.cfg"
            if not config.exists():
                config = root / "scripts/reference.cfg"
            compare(fields["CONFIG_SHA256"], digest(config))
        compare(fields["UPSTREAM"], "14ec60f248547d4d062a1160f0fc26d742319888")
        rows[str(path.relative_to(output))] = dict(fields)
    compare([str(v) for v in sorted(rows)], required)
    if not rows:
        raise WorldCheckError("Missing currency native invocations")
    write_json(output / "native-bindings.json", rows)


def bind_currency(root: Path, output: Path, oracle: Path, layout: Json) -> None:
    cargo_binding(output)
    for group in ("api", "load"):
        for name, item in mapping(at(layout, (group, "cases"))).items():
            positive(root, output / group / name, oracle, item)
            context_binding(output / group / name, api=group == "api")
    guards = mapping(at(layout, ("guards",)))
    compare([str(v) for v in sorted(guards)], [str(v) for v in sorted(GUARDS)])
    compare(
        [
            str(v)
            for v in sorted(p.name for p in (output / "guards").iterdir() if p.is_dir())
        ],
        [str(v) for v in sorted(GUARDS)],
    )
    compare(
        read_json(output / "guards/summary.json"),
        [{"guard": name, "passed": True} for name in GUARDS],
    )
    for name, item in guards.items():
        guard(root, output / "guards" / name, oracle, item)
    invocations(root, output, oracle, at(layout, ("native_invocations",)))
    write_json(
        output / "native-invocations.json",
        {
            "paths": at(layout, ("native_invocations",)),
            "guards": list(GUARDS),
        },
    )


def cargo_binding(output: Path) -> None:
    directory = output / "logs/build"
    command(
        directory,
        [
            "cargo",
            "test",
            "--locked",
            "-p",
            "ottd-sim",
            "--lib",
            "--no-run",
            "--message-format=json",
        ],
    )
    selected = select_executable(
        (directory / "stdout.log").read_text(), "ottd_sim", "lib", test=True
    )
    compare(
        at(read_json(output / "test-binaries.json"), ("ottd_sim",)),
        {
            "original": str(selected),
            "retained": str(output / "bin/ottd_sim"),
            "sha256": digest(selected),
            "kind": ["lib"],
            "profile_test": True,
        },
    )
    compare(digest(selected), digest(output / "bin/ottd_sim"))


def context_binding(directory: Path, *, api: bool) -> None:
    control = read_json(directory / "native/control.json")
    currency = read_json(directory / "native/currency.json")
    current = (
        at(currency, ("before",)) if api else at(currency, ("loads", 0, "context"))
    )
    for field in ("random", "interactive_random"):
        initial = at(control, ("before", field))
        for phase in ("prepared", "after"):
            compare(at(control, (phase, field)), initial)
        compare(at(current, (field,)), initial)
