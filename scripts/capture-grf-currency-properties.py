from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from scripts.currency_properties_ci_run import Mode, run


class Arguments(argparse.Namespace):
    output: Path = Path()


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Capture currency evidence without admitting CI"
    )
    _ = parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args(namespace=Arguments())
    run(Mode.CAPTURE, arguments.output)


if __name__ == "__main__":
    main()
