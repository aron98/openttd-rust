from __future__ import annotations

import os
import tempfile
from pathlib import Path

import pytest

from scripts import currency_properties_ci_run as producer
from scripts.currency_properties_ci_run import (
    Mode,
    admission_layout,
    fresh_build,
    prepare_directory,
    publish,
)
from scripts.grf_control_run import ControlRun
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


def test_normal_admission_refuses_unverified_source_layout(tmp_path: Path) -> None:
    (tmp_path / "scripts").mkdir()
    write_json(
        tmp_path / "scripts/currency-properties-ci-layout.json",
        {"verified_native_corpus": False},
    )
    with pytest.raises(WorldCheckError, match="awaits complete original corpus"):
        _ = admission_layout(tmp_path)


@pytest.mark.parametrize("requested", [None, "relative", "source/nested"])
def test_capture_refuses_nonexternal_output(
    tmp_path: Path, requested: str | None
) -> None:
    root = tmp_path / "source"
    output = None if requested is None else Path(requested)
    if requested == "source/nested":
        output = tmp_path / requested
    with pytest.raises(WorldCheckError, match="absolute and outside source"):
        _ = prepare_directory(root, Mode.CAPTURE, output)
    assert not root.exists()


def test_capture_requires_fresh_output(tmp_path: Path) -> None:
    output = tmp_path / "capture"
    assert prepare_directory(tmp_path / "source", Mode.CAPTURE, output) == output
    with pytest.raises(WorldCheckError, match="must be fresh"):
        _ = prepare_directory(tmp_path / "source", Mode.CAPTURE, output)


@pytest.mark.parametrize(
    "target", ["existing", "relative", "source/target", "proof/../proof/target"]
)
def test_build_refuses_reused_or_internal_target(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, target: str
) -> None:
    directory = tmp_path / "proof"
    root = tmp_path / "source"
    (tmp_path / "existing").mkdir()
    value = target if target == "relative" else str(tmp_path / target)
    monkeypatch.setenv("CARGO_TARGET_DIR", value)
    with pytest.raises(WorldCheckError, match="fresh absolute external target"):
        _ = fresh_build(ControlRun(root, directory, tmp_path / "openttd"))
    assert not directory.exists()


def test_capture_publication_is_false_and_not_pass(tmp_path: Path) -> None:
    output = tmp_path / "capture"
    output.mkdir()
    layout: Json = {"verified_native_corpus": True, "totals": {"guards": 17}}
    publish(
        ControlRun(tmp_path / "source", output, tmp_path / "openttd"),
        layout,
        Mode.CAPTURE,
    )
    assert (
        at(read_json(output / "proposed-layout.json"), ("verified_native_corpus",))
        is False
    )
    assert at(layout, ("verified_native_corpus",)) is True
    assert (output / "summary.txt").read_text().startswith("CAPTURE ")
    assert not (tmp_path / "source").exists()


def test_default_target_is_external_and_environment_is_restored(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    class BeforeCargoError(Exception):
        pass

    root = tmp_path / "source"
    job = ControlRun(root, root / ".artifacts/proof", tmp_path / "openttd")
    monkeypatch.delenv("CARGO_TARGET_DIR", raising=False)
    monkeypatch.setattr(tempfile, "gettempdir", lambda: str(tmp_path / "scratch"))

    def observe_build(actual: ControlRun) -> Path:
        assert actual == job
        target = Path(os.environ["CARGO_TARGET_DIR"])
        assert target.is_absolute()
        assert not target.is_relative_to(root)
        assert not target.exists()
        raise BeforeCargoError

    monkeypatch.setattr(producer, "build_lib", observe_build)
    with pytest.raises(BeforeCargoError):
        _ = fresh_build(job)
    assert "CARGO_TARGET_DIR" not in os.environ
