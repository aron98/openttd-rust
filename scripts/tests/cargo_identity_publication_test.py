from __future__ import annotations

import os
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts import cargo_identity_ci_run as cargo
from scripts.cargo_identity_ci_roster import COMPILER_INPUT, LAYOUT
from scripts.context_ci_support import Selection
from scripts.contract_model import Driver, Scope
from scripts.contract_run import run_driver
from scripts.engine_specs_ci_run import Mode
from scripts.grf_control_run import ControlRun
from scripts.world_check_support import Json, write_json


def completed(*_args: ControlRun | Path | Selection | Json, **_kwargs: str) -> None:
    return


def publication_probe(root: Path) -> None:
    root.mkdir()
    (root / LAYOUT).parent.mkdir(parents=True)
    (root / COMPILER_INPUT).parent.mkdir(parents=True)
    write_json(root / LAYOUT, {"verified_native_corpus": True})
    write_json(root / COMPILER_INPUT, [{"name": "publication-unit", "manifest": {}}])
    oracle = root.parent / "oracle"
    _ = oracle.write_text("unused: subprocess stages are replaced in publication test")

    def binary(_job: ControlRun) -> Path:
        return root.parent / "unused-binary"

    with pytest.MonkeyPatch.context() as patch:
        for name in tuple(os.environ):
            if name.startswith("OTTD_"):
                patch.delenv(name)
        patch.setenv("OTTD_GRF_ORACLE", str(oracle))
        patch.setenv("CARGO_TARGET_DIR", str(root.parent / "fresh-target"))
        patch.setattr(cargo, "__file__", str(root / "scripts/cargo_identity_ci_run.py"))
        for name in (
            "sources",
            "run_exact",
            "run_guards",
            "validate",
            "live_probes",
            "validate_guards",
            "zero",
            "verify_identity",
            "retain",
        ):
            patch.setattr(cargo, name, completed)
        patch.setattr(ControlRun, "provenance", completed)
        patch.setattr(ControlRun, "run", completed)
        patch.setattr(cargo, "build_lib", binary)
        cargo.run(Mode.ADMIT)


def test_actual_publication_is_accepted_by_contract_dispatcher(tmp_path: Path) -> None:
    root = tmp_path / "source"
    driver = Driver(
        "cargo-publication",
        (sys.executable, str(Path(__file__).resolve()), str(root)),
        "source/.artifacts",
        ("summary.txt", "evidence-index.json", "evidence.tar.gz"),
        "PASS private raw cargo identity/translation/road corpus",
        (Scope.RUST,),
        30,
    )
    result = run_driver(driver, tmp_path, tmp_path / "dispatch")
    assert result.exit_code == 0
    assert result.passed, result.error


if __name__ == "__main__":
    publication_probe(Path(sys.argv[1]))
