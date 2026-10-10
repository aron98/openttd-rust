"""Observed nullable caches and archive controls without producers."""

import shutil
from pathlib import Path
from typing import Final

import pytest

from scripts.movement_ci import archive_controls, artifacts, corruption
from scripts.movement_ci.baseline import FileInput
from scripts.movement_ci.protocol import CASES, VehicleId
from scripts.movement_ci.qualify import Qualified
from scripts.movement_ci.runner import Mode, destination
from scripts.movement_ci.value import DECODE, EvidenceError, array, field

ROOT: Final = Path(__file__).resolve().parents[2]


@pytest.mark.parametrize("ordinal", range(4))
def test_actual_nullable_cache_corpus_is_serialized_and_refused(
    tmp_path: Path, ordinal: int
) -> None:
    case = CASES[ordinal]
    stem = case.name.replace("-", "_")
    observed = ROOT / "fixtures/road-movement/validator-inputs" / stem
    physical = DECODE((observed / "initial.physical.json").read_text())
    assert field(array(field(physical, "road_caches"))[0], "first_engine") is None
    runtime = tmp_path / (stem + "_runtime")
    runtime.mkdir()
    _ = shutil.copyfile(
        observed / "initial.physical.json", runtime / "initial.physical.json"
    )
    qualified = Qualified(
        case,
        observed / "unused.sav",
        VehicleId(case.reservations),
        37 if ordinal < 2 else 43,
        74 if ordinal < 2 else 86,
        observed,
        observed,
    )
    corruption.run(tmp_path, qualified)
    corruption.verify(tmp_path, qualified)
    rows = array(
        field(
            DECODE((tmp_path / (stem + "_corruption/summary.json")).read_text()),
            "actual_serialized_refusals",
        )
    )
    assert len(rows) == 26


def test_real_archive_member_and_byte_mutations_are_refused(tmp_path: Path) -> None:
    source = ROOT / "fixtures/road-movement/bus-first.sav"
    output = tmp_path / "proof"
    output.mkdir()
    for name in ("sources", "native-inputs", "baseline-inputs"):
        artifacts.retain(output / name, (FileInput.capture(source),))
    artifacts.seal(output)
    archive_controls.run(output)
    artifacts.seal(output)
    archive_controls.verify(output)
    artifacts.verify(output)


def test_capture_gate_does_not_admit_normal_before_review(tmp_path: Path) -> None:
    with pytest.raises(EvidenceError, match="reviewed"):
        _ = destination(tmp_path, Mode.ADMIT, None, {"verified_native_corpus": False})
