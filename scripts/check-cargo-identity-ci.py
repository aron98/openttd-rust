# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: PYTHONPATH=. python3 scripts/check-cargo-identity-ci.py
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.cargo_identity_ci_run import run
from scripts.engine_specs_ci_run import Mode

if __name__ == "__main__":
    run(Mode.ADMIT)
