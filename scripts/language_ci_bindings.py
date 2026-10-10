from __future__ import annotations

import hashlib
import json
from pathlib import Path

from scripts.context_ci_support import native_bindings
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import sequence, text
from scripts.language_ci_compare import compare
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


def normalized(value: Json, roots: tuple[Path, Path, Path]) -> Json:
    root, directory, oracle = roots
    match value:
        case str() as raw:
            for path, token in [
                (directory, "$CASE"),
                (oracle, "$ORACLE"),
                (root, "$ROOT"),
            ]:
                if raw == str(path) or raw.startswith(f"{path}/"):
                    return token + raw[len(str(path)) :]
                if "=" in raw:
                    key, tail = raw.split("=", 1)
                    if tail == str(path) or tail.startswith(f"{path}/"):
                        return key + "=" + token + tail[len(str(path)) :]
            return raw
        case list() as values:
            return [normalized(row, roots) for row in values]
        case dict() as fields:
            return {key: normalized(row, roots) for key, row in fields.items()}
        case None | bool() | int() | float():
            return value


def manifest_digest(value: Json, roots: tuple[Path, Path, Path]) -> str:
    encoded = json.dumps(
        normalized(value, roots), sort_keys=True, separators=(",", ":"), allow_nan=False
    ).encode()
    return hashlib.sha256(encoded).hexdigest()


def bound_inputs(directory: Path, layout: Json, roots: tuple[Path, Path, Path]) -> None:
    compare(
        manifest_digest(read_json(directory / "manifest.json"), roots),
        at(layout, ("manifest_sha256",)),
    )
    for name, sha in at_mapping(layout, "inputs").items():
        path = directory / name
        if (
            not path.resolve().is_relative_to(directory.resolve())
            or digest(path) != sha
        ):
            raise WorldCheckError("Language input source changed")


def at_mapping(value: Json, key: str) -> dict[str, Json]:
    match at(value, (key,)):
        case dict() as result:
            return result
        case _:
            raise WorldCheckError("Missing language binding object")


def hash_rows(path: Path, expected: list[Path]) -> None:
    lines = path.read_text().splitlines()
    if len(lines) != len(expected):
        raise WorldCheckError("Language native input membership differs")
    for row, source in zip(lines, expected, strict=True):
        sha, separator, name = row.partition("  ")
        if not separator or name != str(source) or sha != digest(source):
            raise WorldCheckError("Language native input hash differs")


def positive_binding(root: Path, directory: Path, oracle: Path, layout: Json) -> Json:
    roots = root, directory, oracle
    bound_inputs(directory, layout, roots)
    compare(
        normalized(read_json(directory / "argv.json"), roots), at(layout, ("argv",))
    )
    manifest = read_json(directory / "manifest.json")
    files = [
        Path(text(at(row, ("path",)))) for row in sequence(at(manifest, ("files",)))
    ]
    packs = [
        Path(text(row)) / "input.lng"
        for row in sequence(at(manifest, ("language", "pack_directories")))
    ]
    hash_rows(directory / "native/inputs.sha256", files)
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
        raise WorldCheckError("Original baseline source escaped immutable oracle")
    hash_rows(directory / "native/baseline-inputs.sha256", baseline)
    return {
        "manifest": digest(directory / "manifest.json"),
        "sources": {str(path): digest(path) for path in [*files, *packs, *baseline]},
    }


def guard_binding(root: Path, directory: Path, oracle: Path, layout: Json) -> Json:
    roots = root, directory, oracle
    bound_inputs(directory, layout, roots)
    compare(
        normalized(read_json(directory / "argv.json"), roots), at(layout, ("argv",))
    )
    compare(read_json(directory / "status.json"), at(layout, ("status",)))
    diagnostic = (directory / "stderr.log").read_text()
    native_log = directory / "native/stderr.log"
    if native_log.exists():
        diagnostic += native_log.read_text()
    if text(at(layout, ("diagnostic",))) not in diagnostic:
        raise WorldCheckError("Language guard diagnostic missing")
    if (directory / "native/language.json").exists():
        raise WorldCheckError("Language guard emitted observation")
    output = directory / "language.json"
    if directory.name == "duplicate-output":
        if output.read_bytes() != b"sentinel\n":
            raise WorldCheckError("Language duplicate output overwritten")
    elif output.exists():
        raise WorldCheckError("Language guard emitted alternate observation")
    stale_sources(root, directory)
    return {
        "argv": read_json(directory / "argv.json"),
        "status": read_json(directory / "status.json"),
        "native_log_present": native_log.exists(),
        "native_sha256": digest(oracle),
    }


def stale_sources(root: Path, directory: Path) -> None:
    if directory.name != "stale":
        return
    for path in (directory / "stale-source/reference").iterdir():
        expected = (root / "reference" / path.name).read_bytes()
        if path.name == "grf_language.hpp":
            expected += b"\n"
        if path.read_bytes() != expected:
            raise WorldCheckError("Stale guard changed unexpected source")


def bind_language(root: Path, output: Path, oracle: Path, layout: Json) -> None:
    native_bindings(root, output, oracle)
    cases = at_mapping(layout, "case_bindings")
    positive = {
        name: positive_binding(root, output / "results" / name, oracle, item)
        for name, item in cases.items()
    }
    guards = at_mapping(layout, "guard_bindings")
    if len(cases) != 110 or len(guards) != 13:
        raise WorldCheckError("Incomplete language invocation set")
    refused = {
        name: guard_binding(root, output / "guards" / name, oracle, item)
        for name, item in guards.items()
    }
    expected_invocations = at(layout, ("native_invocations",))
    actual = sorted(read_bindings(output))
    compare([str(path) for path in actual], expected_invocations)
    write_json(
        output / "native-invocations.json", {"cases": positive, "guards": refused}
    )


def read_bindings(output: Path) -> dict[str, Json]:
    match read_json(output / "native-bindings.json"):
        case dict() as values:
            return values
        case _:
            raise WorldCheckError("Missing native invocation evidence")
