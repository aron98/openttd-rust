# /// script
# requires-python = ">=3.11"
# dependencies = ["pytest"]
# ///
# Run: uv run --with pytest python -m pytest scripts/tests/evidence_transport_test.py
from __future__ import annotations

import shutil
import sys
from pathlib import Path
from urllib.parse import unquote

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.contract_model import Driver, Scope
from scripts.contract_run import run_driver
from scripts.gameplay_foundations import FoundationRun, log_name
from scripts.grf_control_archive import archive_files
from scripts.grf_control_run import ControlRun
from scripts.world_check_support import WorldCheckError, read_json


@pytest.mark.parametrize("runner", [FoundationRun, ControlRun])
def test_real_runner_preserves_selector_in_portable_copied_evidence(
    tmp_path: Path, runner: type[FoundationRun | ControlRun]
) -> None:
    name = "commands::road_depot::occupancy::native::native_ground_cutoff"
    argv = [sys.executable, "-c", "import sys; print(sys.argv[1])", name]
    output = tmp_path / "sloped"
    job = runner(tmp_path, output, tmp_path / "unused-oracle")

    result = job.run(name, argv)

    mapped = name.replace(":", "%3A")
    directory = output / "logs" / mapped
    assert directory.is_dir()
    assert result.stdout.strip() == name
    assert read_json(directory / "argv.json") == argv
    assert unquote(mapped) == name
    copied = Path(shutil.copytree(output, tmp_path / "corruption"))
    assert (copied / "logs" / mapped / "argv.json").read_bytes() == (
        directory / "argv.json"
    ).read_bytes()
    assert all(
        ":" not in str(path.relative_to(tmp_path)) for path in tmp_path.rglob("*")
    )


def test_encoded_label_does_not_collide_with_literal_percent_label(
    tmp_path: Path,
) -> None:
    job = ControlRun(tmp_path, tmp_path / "proof", tmp_path / "oracle")
    argv = [sys.executable, "-c", "print('proof')"]

    _ = job.run("a::b", argv)
    _ = job.run("a%3A%3Ab", argv)

    assert (job.output / "logs/a%3A%3Ab/argv.json").is_file()
    assert (job.output / "logs/a%253A%253Ab/argv.json").is_file()


@pytest.mark.parametrize("character", list('\\:"<>|*?\r\n'))
def test_log_label_mapping_preserves_nested_identity(character: str) -> None:
    original = f"case/nested{character}label%"

    mapped = log_name(original)

    assert unquote(mapped) == original
    assert mapped.startswith("case/")
    assert character not in mapped


def test_archive_rejects_remote_failed_path_before_packaging(tmp_path: Path) -> None:
    path = (
        tmp_path
        / "corruption/logs"
        / "commands::road_depot::occupancy::native::native_ground_cutoff"
        / "argv.json"
    )
    path.parent.mkdir(parents=True)
    _ = path.write_text('["actual", "original-selector"]')

    with pytest.raises(WorldCheckError, match="portable"):
        archive_files(tmp_path, [path])

    assert not (tmp_path / "evidence.tar.gz").exists()


def test_failed_driver_emits_bounded_diagnostic_without_changing_raw_stderr(
    tmp_path: Path, capsys: pytest.CaptureFixture[str]
) -> None:
    payload = "x" * 10_000 + "\nACTUAL_PRIMARY_FAILURE\n"
    driver = Driver(
        "broken-driver",
        (
            sys.executable,
            "-c",
            f"import sys; sys.stderr.write({payload!r}); sys.exit(7)",
        ),
        None,
        (),
        "PASS",
        (Scope.RUST,),
        10,
    )

    result = run_driver(driver, tmp_path, tmp_path / "run")

    diagnostic = capsys.readouterr().err
    assert not result.passed
    assert result.exit_code == 7
    assert "broken-driver" in diagnostic
    assert "7" in diagnostic
    assert "ACTUAL_PRIMARY_FAILURE" in diagnostic
    assert len(diagnostic) < 5000
    assert (tmp_path / "run/stderr.log").read_text() == payload
