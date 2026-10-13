# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: PYTHONPATH=. python3 scripts/capture-cargo-identity.py --output ABSOLUTE_NEW_DIR
import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.cargo_identity_ci_run import run
from scripts.engine_specs_ci_run import Mode


class Arguments(argparse.Namespace):
    output: Path = Path()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    _ = parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args(namespace=Arguments())
    run(Mode.CAPTURE, args.output)
