"""Capture passive table-owner graphs for the real Rust lifetime consumers."""

import json
import sys
from pathlib import Path
from typing import Final

from .compare import digest, run

GRAPH_SESSIONS: Final = ("ordinary", "gc", "gc-own", "marked", "mutations")
LIFETIME_SESSIONS: Final = ("capture", "default", "error")


class DelegateOwnerError(RuntimeError):
    """The passive recorder changed endpoints or failed to observe real frees."""


def endpoints(output: bytes) -> tuple[bytes, ...]:
    return tuple(line for line in output.splitlines() if not line.startswith(b"free "))


def capture(native: Path, output: Path) -> None:
    output.mkdir()
    source_root = Path(__file__).resolve().parents[4]
    fixtures = source_root / "crates/ottd-script/tests/delegate-lifetimes"
    for name in (*GRAPH_SESSIONS, *LIFETIME_SESSIONS):
        source = fixtures / f"{name}.nut"
        arguments = (
            ["--delegate-cycle-owner", name, str(source)]
            if name in LIFETIME_SESSIONS
            else ["--delegate-owner", name]
        )
        active = run([str(native), *arguments, "on"], output / f"{name}-on")
        passive = run([str(native), *arguments, "off"], output / f"{name}-off")
        if endpoints(active) != tuple(passive.splitlines()):
            raise DelegateOwnerError(f"Recorder changed graph endpoints: {name}")
        if not any(line.startswith(b"free ") for line in active.splitlines()):
            raise DelegateOwnerError(f"Recorder omitted all retirements: {name}")
        for mode in ("on", "off"):
            if (output / f"{name}-{mode}.stderr").read_bytes():
                raise DelegateOwnerError(f"Native owner diagnostic: {name}/{mode}")
        _ = (output / f"{name}.txt").write_bytes(active)
    _ = (output / "authority.json").write_text(
        json.dumps(
            {
                "native_sha256": digest(native),
                "sources": {
                    name: digest(fixtures / f"{name}.nut")
                    for name in LIFETIME_SESSIONS
                },
                "graph_sessions": GRAPH_SESSIONS,
                "lifetime_sessions": LIFETIME_SESSIONS,
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    print("PASS eight original delegate owner graphs and recorder neutrality")


if __name__ == "__main__":
    capture(Path(sys.argv[1]).resolve(strict=True), Path(sys.argv[2]).resolve())
