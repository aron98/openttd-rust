from __future__ import annotations

from pathlib import Path

from scripts.context_ci_support import native_bindings
from scripts.depot_build_archive import bounded_paths
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import sequence, text
from scripts.language_ci_bindings import bound_inputs, hash_rows, normalized
from scripts.language_ci_compare import compare, mapping
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


def positive(root: Path, directory: Path, oracle: Path, layout: Json) -> None:
    roots = root, directory, oracle
    bound_inputs(directory, layout, roots)
    compare(
        normalized(read_json(directory / "argv.json"), roots), at(layout, ("argv",))
    )
    manifest = read_json(directory / "manifest.json")
    files = [
        Path(text(at(row, ("path",)))) for row in sequence(at(manifest, ("files",)))
    ]
    hashes = directory / "native/inputs.sha256"
    expected = [
        f"{digest(path) if path.exists() else 'MISSING'}  {path}" for path in files
    ]
    actual = hashes.read_text().splitlines() if hashes.exists() else []
    compare([str(v) for v in actual], [str(v) for v in expected])
    packs = [
        Path(text(v)) / "input.lng"
        for v in sequence(at(manifest, ("language", "pack_directories")))
    ]
    hash_rows(directory / "native/language-inputs.sha256", packs)
    baseline = [
        Path(text(v))
        for v in sequence(
            at(read_json(directory / "native/control.json"), ("baseline_sources",))
        )
    ]
    if len(baseline) != 2 or any(
        not path.resolve().is_relative_to(oracle.parent / "baseset")
        for path in baseline
    ):
        raise WorldCheckError("String baseline sources escaped immutable oracle")
    hash_rows(directory / "native/baseline-inputs.sha256", baseline)


def guard(root: Path, directory: Path, oracle: Path, layout: Json) -> None:
    _ = bounded_paths(directory, {text(v) for v in sequence(at(layout, ("paths",)))})
    bound_inputs(directory, layout, (root, directory, oracle))
    compare(
        normalized(read_json(directory / "argv.json"), (root, directory, oracle)),
        at(layout, ("argv",)),
    )
    compare(read_json(directory / "status.json"), at(layout, ("status",)))
    errors = (directory / "stderr.log").read_text()
    native = directory / "native/stderr.log"
    if native.exists():
        errors += native.read_text()
    if text(at(layout, ("diagnostic",))) not in errors:
        raise WorldCheckError("Missing string guard diagnostic")
    output = directory / "strings.json"
    if directory.name == "duplicate":
        compare(output.read_text(), "sentinel\n")
    elif output.exists():
        raise WorldCheckError("String guard emitted alternate observation")
    if (directory / "native/strings.json").exists():
        raise WorldCheckError("String guard emitted observation")
    stale_sources(root, directory)


def stale_sources(root: Path, directory: Path) -> None:
    if directory.name != "stale":
        return
    for folder in ("scripts", "reference"):
        for path in (directory / "stale-source" / folder).iterdir():
            expected = (root / folder / path.name).read_bytes()
            if folder == "reference" and path.name == "grf_strings.hpp":
                expected += b"\n"
            if path.read_bytes() != expected:
                raise WorldCheckError("String stale fixture changed unexpected source")


def bind_strings(root: Path, output: Path, oracle: Path, layout: Json) -> None:
    native_bindings(root, output, oracle)
    actual = sorted(mapping(read_json(output / "native-bindings.json")))
    compare([str(v) for v in actual], at(layout, ("native_invocations",)))
    for group in ("api", "load"):
        for name, item in mapping(at(layout, (group, "cases"))).items():
            positive(root, output / group / name, oracle, item)
    expected = mapping(at(layout, ("guards",)))
    compare(
        [str(v) for v in sorted(expected)],
        ["duplicate", "non-save", "replay", "reused", "stale", "subset", "unarmed"],
    )
    compare(
        [
            str(v)
            for v in sorted(
                path.name for path in (output / "guards").iterdir() if path.is_dir()
            )
        ],
        [str(v) for v in sorted(expected)],
    )
    for name, item in expected.items():
        guard(root, output / "guards" / name, oracle, item)
    write_json(
        output / "native-invocations.json",
        {"cases": 100, "guards": 7, "paths": [str(v) for v in actual]},
    )
