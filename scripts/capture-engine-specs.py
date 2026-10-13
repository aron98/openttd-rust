from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.engine_specs_ci_run import Mode, run


class Arguments(argparse.Namespace):
    output: Path = Path()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        description="Capture private raw engine evidence without admitting CI"
    )
    _ = parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args(namespace=Arguments())
    run(Mode.CAPTURE, args.output)
