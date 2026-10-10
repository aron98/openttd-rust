# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""Normal complete movement CI, or an explicit fresh external capture."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from movement_ci import admission, archive_controls, artifacts
from movement_ci.runner import Mode, run
from movement_ci.value import EvidenceError


def main() -> None:
    root = Path(__file__).resolve().parents[1]
    if len(sys.argv) == 1:
        run(root, Mode.ADMIT, None)
        return
    if len(sys.argv) == 3 and sys.argv[1] == "--verify":
        output = Path(sys.argv[2])
        if not output.is_absolute() or not output.is_dir():
            raise EvidenceError("existing absolute corpus required")
        admission.validate(output)
        archive_controls.verify(output)
        artifacts.verify(output)
        print("PASS movement recorded corpus admission")
        return
    if len(sys.argv) == 3 and sys.argv[1] == "--capture":
        run(root, Mode.CAPTURE, Path(sys.argv[2]))
        return
    raise EvidenceError(
        "usage: check-road-movement-ci.py [--capture DIR | --verify DIR]"
    )


if __name__ == "__main__":
    main()
