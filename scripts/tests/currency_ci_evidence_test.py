from __future__ import annotations

from pathlib import Path

import pytest

from scripts.context_ci_support import sources
from scripts.currency_ci_bindings import compiler_inputs, context_binding
from scripts.currency_ci_compare import controls, expected_controls, project_rows
from scripts.currency_ci_evidence import case, fingerprint, stable_context
from scripts.currency_ci_probes import selected_binary
from scripts.currency_ci_roster import COMPILER_INPUTS
from scripts.gameplay_foundations import digest
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    read_json,
    replace,
    write_json,
)


def test_complete_scalar_and_array_controls_are_required() -> None:
    value: Json = [{"owner": 7, "pending": [], "missing": None}]
    rows = expected_controls(value)
    assert controls(value, rows) == len(rows)
    with pytest.raises(WorldCheckError):
        _ = controls(value, rows[:-1])
    with pytest.raises(WorldCheckError):
        _ = controls(value, [*rows, rows[0]])


def test_false_or_ineffective_controls_are_rejected() -> None:
    variations: list[Json] = [
        {"path": "", "altered": 7, "rejected": True},
        {"path": "", "altered": None, "rejected": False},
    ]
    for row in variations:
        with pytest.raises(WorldCheckError):
            _ = controls(7, [row])


def test_projection_keeps_all_configured_decisions_and_stage_boundaries() -> None:
    state: dict[str, Json] = {"owners": [1], "pending": [], "strings": {}}
    raw: Json = {
        "events": [
            {**state, "phase": "decision", "record": {"file": 0}},
            {
                **state,
                "phase": "decision",
                "record": {
                    "stage": 5,
                    "file": 2,
                    "line": 9,
                    "offset": 17,
                },
            },
            {**state, "phase": "stage-end", "stage": 5},
        ]
    }
    assert project_rows(raw, 2) == [
        {
            "coordinates": {"stage": 5, "file": 0, "line": 9, "offset": 17},
            "state": state,
        },
        {
            "coordinates": {"stage": 5, "file": 0, "line": 0, "offset": 0},
            "state": state,
        },
    ]
    with pytest.raises(WorldCheckError):
        _ = project_rows({"events": [{"phase": "invented"}]}, 2)


def fixture(directory: Path) -> tuple[Json, Json, Json]:
    rust: Json = [{"owners": [{"name": 7}], "pending": [], "strings": []}]
    native: Json = {
        "mode": "separate-process-original-currency-api",
        "arms": 1,
        "consumptions": 1,
        "explicit_fixture_reset": True,
        "before": {"interactive_random": [1, 1]},
        "after": {"interactive_random": [1, 1]},
        "results": rust,
    }
    (directory / "native").mkdir()
    baseset = directory / "bundle/baseset"
    baseset.mkdir(parents=True)
    baseline = [baseset / "OPENTTD.GRF", baseset / "extra.grf"]
    for path in baseline:
        _ = path.write_bytes(path.name.encode())
    native["baseline_sources"] = [str(path) for path in baseline]
    write_json(
        directory / "native/control.json",
        {"baseline_sources": native["baseline_sources"]},
    )
    _ = (directory / "native/baseline-inputs.sha256").write_text(
        "".join(f"{digest(path)}  {path}\n" for path in baseline)
    )
    write_json(directory / "native/currency.json", native)
    write_json(directory / "rust.json", rust)
    rows = expected_controls(rust)
    write_json(directory / "controls.json", rows)
    write_json(directory / "status.json", 0)
    layout: Json = {
        "paths": [
            "native/currency.json",
            "rust.json",
            "controls.json",
            "status.json",
            "native/control.json",
            "native/baseline-inputs.sha256",
            "bundle/baseset/OPENTTD.GRF",
            "bundle/baseset/extra.grf",
        ],
        "native_observables_sha256": fingerprint(
            stable_context(native), (directory, directory, directory / "bundle")
        ),
        "coverage": {"operations": 1, "controls": len(rows)},
    }
    return native, rust, layout


def test_paired_native_and_rust_change_is_rejected(tmp_path: Path) -> None:
    native, rust, layout = fixture(tmp_path)
    assert case(tmp_path, tmp_path, tmp_path / "bundle/openttd", layout, api=True)
    replace(native, ("results", 0, "owners", 0, "name"), 8)
    replace(rust, (0, "owners", 0, "name"), 8)
    write_json(tmp_path / "native/currency.json", native)
    write_json(tmp_path / "rust.json", rust)
    with pytest.raises(WorldCheckError):
        _ = case(tmp_path, tmp_path, tmp_path / "bundle/openttd", layout, api=True)


def test_omitted_or_extra_raw_member_is_rejected(tmp_path: Path) -> None:
    _, _, layout = fixture(tmp_path)
    _ = (tmp_path / "extra.txt").write_text("unadmitted\n")
    with pytest.raises(WorldCheckError):
        _ = case(tmp_path, tmp_path, tmp_path / "bundle/openttd", layout, api=True)
    (tmp_path / "extra.txt").unlink()
    (tmp_path / "status.json").unlink()
    with pytest.raises(WorldCheckError):
        _ = case(tmp_path, tmp_path, tmp_path / "bundle/openttd", layout, api=True)


def test_fingerprint_normalizes_roots_but_preserves_owner_values(
    tmp_path: Path,
) -> None:
    a = tmp_path / "a"
    b = tmp_path / "b"
    original: Json = {"path": str(a / "input.grf"), "owner": {"name": 7}}
    moved: Json = {"path": str(b / "input.grf"), "owner": {"name": 7}}
    assert fingerprint(original, (a, a, a)) == fingerprint(moved, (b, b, b))
    changed: Json = {"path": str(b / "input.grf"), "owner": {"name": 8}}
    assert fingerprint(original, (a, a, a)) != fingerprint(changed, (b, b, b))


def test_startup_entropy_is_process_local_and_game_rng_stays_exact() -> None:
    def observation(seed: int) -> Json:
        context: Json = {"interactive_random": [seed, seed], "random": [3, 4]}
        return {
            "mode": "separate-process-original-currency-api",
            "before": context,
            "after": {"interactive_random": [seed, seed], "random": [3, 4]},
        }

    original = observation(1)
    changed_start = observation(2)
    assert stable_context(original) == stable_context(changed_start)
    assert original == observation(1)
    replace(changed_start, ("after", "interactive_random", 0), 3)
    with pytest.raises(WorldCheckError):
        _ = stable_context(changed_start)
    changed_game = observation(1)
    replace(changed_game, ("after", "random", 0), 9)
    assert stable_context(original) != stable_context(changed_game)


def test_all_reload_contexts_require_the_same_bounded_seed() -> None:
    context: Json = {"interactive_random": [1, 2]}
    raw: Json = {
        "mode": "actual-loader-currency-owners",
        "loads": [{"context": context}, {"context": context}],
        "reload_context": {"before": context, "prepared": context, "after": context},
    }
    assert stable_context(raw)
    replace(raw, ("reload_context", "prepared"), {"interactive_random": [1, 3]})
    with pytest.raises(WorldCheckError):
        _ = stable_context(raw)
    invalid_pairs: list[Json] = [[1], [1, 2, 3], [True, 2], [-1, 2], [4294967296, 2]]
    for value in invalid_pairs:
        invalid: Json = {
            "mode": "separate-process-original-currency-api",
            "before": {"interactive_random": value},
            "after": {"interactive_random": value},
        }
        with pytest.raises(WorldCheckError):
            _ = stable_context(invalid)


def test_control_rng_is_bound_before_and_after_the_actual_loader(
    tmp_path: Path,
) -> None:
    (tmp_path / "native").mkdir()
    context: Json = {"random": [3, 4], "interactive_random": [1, 2]}
    raw: Json = {"before": context, "prepared": context, "after": context}
    write_json(tmp_path / "native/control.json", raw)
    write_json(tmp_path / "native/currency.json", {"loads": [{"context": context}]})
    context_binding(tmp_path, api=False)
    replace(raw, ("after",), {"random": [3, 4], "interactive_random": [1, 3]})
    write_json(tmp_path / "native/control.json", raw)
    with pytest.raises(WorldCheckError):
        context_binding(tmp_path, api=False)


def test_retained_executable_probe_restores_read_only_bytes_and_mode(
    tmp_path: Path,
) -> None:
    (tmp_path / "bin").mkdir()
    (tmp_path / "corruption").mkdir()
    original = tmp_path / "cargo-original"
    retained = tmp_path / "bin/ottd_sim"
    for path in (original, retained):
        _ = path.write_bytes(b"original executable fixture")
        path.chmod(0o555)
    native = tmp_path / "openttd"
    _ = native.write_bytes(b"native fixture")
    stamp = tmp_path / "replay-build.sha256"
    _ = stamp.write_bytes(b"stamp fixture")
    write_json(
        tmp_path / "provenance.json",
        {
            "source_hashes": {"cargo-original": digest(original)},
            "native_binary": str(native),
            "native_sha256": digest(native),
            "stamp_sha256": digest(stamp),
        },
    )
    write_json(
        tmp_path / "test-binaries.json",
        {
            "ottd_sim": {
                "original": str(original),
                "retained": str(retained),
                "sha256": digest(original),
                "kind": ["lib"],
                "profile_test": True,
            }
        },
    )
    rows: list[Json] = []
    selected_binary(tmp_path, tmp_path / "corruption", tmp_path, rows)
    assert len(rows) == 1
    assert retained.read_bytes() == original.read_bytes()
    assert retained.stat().st_mode & 0o777 == 0o555
    assert original.stat().st_mode & 0o777 == 0o555


def test_embedded_compiler_inputs_are_bound_before_execution(tmp_path: Path) -> None:
    write_json(tmp_path / "provenance.json", {"source_hashes": {}, "retained": True})
    for name in COMPILER_INPUTS:
        path = tmp_path / name
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_bytes(b"original compile input")
    compiler_inputs(tmp_path, tmp_path)
    result = read_json(tmp_path / "provenance.json")
    assert at(result, ("retained",)) is True
    for name in COMPILER_INPUTS:
        assert at(result, ("source_hashes", name)) == digest(tmp_path / name)
    pinned: Json = {"sources": at(result, ("source_hashes",))}
    sources(tmp_path, pinned)
    _ = (tmp_path / COMPILER_INPUTS[0]).write_bytes(b"changed compile input")
    with pytest.raises(WorldCheckError):
        sources(tmp_path, pinned)
