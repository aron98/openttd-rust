"""Capture finite native shared-root sessions with mandatory observer source copies."""

import json
import shutil
import stat
import sys
from pathlib import Path

from scripts.script_vm_inputs import OBSERVER_INPUTS
from scripts.script_vm_native_roots_audit import PHASES, active_pair, recorder_pair
from scripts.script_vm_provenance import verify_observer_sources

from .compare import digest, run


def capture(native: Path, output: Path) -> None:
    """Run only the declared finite commands, preserving every raw channel."""
    root = Path(__file__).resolve().parents[4]
    verify_observer_sources(root, native.parent)
    output.mkdir()
    copied = output / "native"
    _ = shutil.copy2(native, copied)
    sources = output / "observer-sources"
    sources.mkdir()
    for name in OBSERVER_INPUTS:
        _ = shutil.copy2(native.parent / name, sources / name)
    verify_observer_sources(root, sources)
    for mode in PHASES:
        values = tuple(
            run(
                [str(copied), "--native-roots", mode, recorder],
                output / f"{mode}-{recorder}",
            )
            for recorder in ("off", "on")
        )
        _ = recorder_pair(mode, *values)
    fixtures = Path(__file__).parent / "fixtures/active"
    for name in ("frame", "varargs", "error", "closure", "allocated-receiver"):
        source = fixtures / f"{name}.nut"
        for action in ("off", "on"):
            values = tuple(
                run(
                    [
                        str(copied),
                        "--native-roots-active",
                        str(source),
                        action,
                        recorder,
                    ],
                    output / f"active-{name}-{action}-{recorder}",
                )
                for recorder in ("off", "on")
            )
            legacy = run(
                [str(copied), "--active-owner", "script", str(source), action, "on"],
                output / f"legacy-{name}-{action}",
            )
            _ = active_pair(values[0], values[1], legacy)
    _ = (output / "authority.json").write_text(
        json.dumps(
            {
                "native_sha256": digest(copied),
                "sources": {
                    name: {
                        "sha256": digest(sources / name),
                        "mode": stat.S_IMODE((sources / name).stat().st_mode),
                    }
                    for name in OBSERVER_INPUTS
                },
                "scenarios": list(PHASES),
                "new_native_processes": 40,
                "legacy_active_controls": 10,
            },
            indent=2,
        )
        + "\n"
    )
    print("PASS 20 finite roots +20 active snapshots +10 unchanged legacy controls")


if __name__ == "__main__":
    capture(Path(sys.argv[1]).resolve(strict=True), Path(sys.argv[2]).resolve())
