from __future__ import annotations

from copy import deepcopy
from pathlib import Path

import pytest

from scripts.currency_ci_baseline import bind_baseline
from scripts.currency_ci_evidence import fingerprint
from scripts.gameplay_foundations import digest
from scripts.world_check_support import Json, WorldCheckError, write_json


def observation(builtin: Path, extra: Path) -> dict[str, Json]:
    return {
        "mode": "separate-process-original-currency-api",
        "baseline_sources": [str(builtin), str(extra)],
        "owners": [{"prefix": "openttd.grf", "rate": 2}],
    }


def test_only_first_known_builtin_basename_is_canonicalized(tmp_path: Path) -> None:
    oracle = tmp_path / "oracle"
    builtin = oracle / "baseset/openttd.grf"
    extra = oracle / "baseset/opengfx/extra.grf"
    lower = observation(builtin, extra)
    original = deepcopy(lower)
    upper = observation(builtin.with_name("OPENTTD.GRF"), extra)
    roots = tmp_path, tmp_path / "case", oracle
    assert fingerprint(lower, roots) == fingerprint(upper, roots)
    assert lower == original
    changed: list[Json] = [
        observation(oracle / "BASESET/openttd.grf", extra),
        observation(oracle / "other/openttd.grf", extra),
        observation(builtin.with_name("Openttd.grf"), extra),
        observation(builtin.with_name("arbitrary.grf"), extra),
        observation(builtin, extra.with_name("EXTRA.GRF")),
        {**lower, "owners": [{"prefix": "OPENTTD.GRF", "rate": 2}]},
        {**lower, "owners": [{"prefix": "openttd.grf", "rate": 3}]},
        {**lower, "baseline_sources": [str(extra), str(builtin)]},
    ]
    for value in changed:
        assert fingerprint(value, roots) != fingerprint(lower, roots)


def test_unrelated_json_and_nested_sources_are_not_casefolded(tmp_path: Path) -> None:
    builtin = tmp_path / "oracle/baseset/openttd.grf"
    upper = builtin.with_name("OPENTTD.GRF")
    roots = tmp_path, tmp_path / "case", tmp_path / "oracle"
    for key in ("baseline_sources", "nested"):
        assert fingerprint({key: [str(builtin)]}, roots) != fingerprint(
            {key: [str(upper)]}, roots
        )


@pytest.fixture
def bound(tmp_path: Path) -> tuple[Path, Path, dict[str, Json]]:
    directory = tmp_path / "case"
    (directory / "native").mkdir(parents=True)
    oracle = tmp_path / "oracle/openttd"
    builtin = oracle.parent / "baseset/openttd.grf"
    builtin.parent.mkdir(parents=True)
    extra = builtin.with_name("extra.grf")
    _ = builtin.write_bytes(b"known builtin content")
    _ = extra.write_bytes(b"known extra content")
    native = observation(builtin, extra)
    write_json(directory / "native/control.json", native)
    _ = (directory / "native/baseline-inputs.sha256").write_text(
        "".join(f"{digest(path)}  {path}\n" for path in (builtin, extra))
    )
    bind_baseline(directory, oracle, native)
    return directory, oracle, native


def test_currency_and_control_baselines_must_match_raw_order(
    bound: tuple[Path, Path, dict[str, Json]],
) -> None:
    directory, oracle, native = bound
    builtin = oracle.parent / "baseset/openttd.grf"
    changed = observation(
        builtin.with_name("OPENTTD.GRF"), builtin.with_name("extra.grf")
    )
    with pytest.raises(WorldCheckError):
        bind_baseline(directory, oracle, changed)
    changed["baseline_sources"] = [str(builtin.with_name("extra.grf")), str(builtin)]
    write_json(directory / "native/control.json", changed)
    with pytest.raises(WorldCheckError):
        bind_baseline(directory, oracle, changed)
    with pytest.raises(WorldCheckError):
        bind_baseline(directory, oracle, {"mode": native["mode"]})


def test_baseline_content_and_hash_order_are_bound(
    bound: tuple[Path, Path, dict[str, Json]],
) -> None:
    directory, oracle, native = bound
    proof = directory / "native/baseline-inputs.sha256"
    original = proof.read_text()
    _ = proof.write_text("\n".join(reversed(original.splitlines())) + "\n")
    with pytest.raises(WorldCheckError):
        bind_baseline(directory, oracle, native)
    _ = proof.write_text(original)
    _ = (oracle.parent / "baseset/openttd.grf").write_bytes(b"wrong content")
    with pytest.raises(WorldCheckError):
        bind_baseline(directory, oracle, native)


def test_matching_raw_paths_cannot_escape_oracle(
    bound: tuple[Path, Path, dict[str, Json]],
) -> None:
    directory, oracle, native = bound
    builtin = oracle.parent / "baseset/openttd.grf"
    outside = directory / "outside.grf"
    _ = outside.write_bytes(b"outside")
    changed = observation(builtin, outside)
    write_json(directory / "native/control.json", changed)
    with pytest.raises(WorldCheckError):
        bind_baseline(directory, oracle, changed)
    write_json(directory / "native/control.json", native)
    builtin.unlink()
    builtin.symlink_to(outside)
    with pytest.raises(WorldCheckError):
        bind_baseline(directory, oracle, native)
