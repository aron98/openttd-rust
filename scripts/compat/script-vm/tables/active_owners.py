"""Capture intentional active collection separately from passive recorder controls."""

import json
import sys
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Final

from .compare import digest, run
from .delegate_owners import endpoints


@dataclass(frozen=True, slots=True)
class Session:
    name: str
    mode: str
    fixture: str


SESSIONS: Final = (
    Session("local", "local", "local"),
    Session("local-array", "local-array", "local"),
    Session("local-native", "local-native", "local"),
    Session("host", "host", "local"),
    Session("frame", "script", "frame"),
    Session("varargs", "script", "varargs"),
    Session("error", "script", "error"),
    Session("closure", "script", "closure"),
    Session("weak-bound", "script", "weak-bound"),
    Session("weak-expired", "script", "weak-expired"),
    Session("allocated-stack", "script", "allocated-stack"),
    Session("allocated-tail", "script", "allocated-tail"),
    Session("allocated-receiver", "script", "allocated-receiver"),
    Session("uncaught", "handler", "uncaught"),
)


class ActiveOwnerError(RuntimeError):
    """The active action or passive observer violated its declared protocol."""


def capture(native: Path, output: Path) -> None:
    output.mkdir()
    fixtures = Path(__file__).parent / "fixtures/active"
    for session in SESSIONS:
        source = fixtures / f"{session.fixture}.nut"
        for action in ("off", "on"):
            observations: list[bytes] = []
            for recorder in ("off", "on"):
                arguments = [
                    "--active-owner",
                    session.mode,
                    str(source),
                    action,
                    recorder,
                ]
                if session.mode == "handler":
                    arguments.append(str(fixtures / "handler.nut"))
                prefix = output / f"{session.name}-{action}-{recorder}"
                observation = run([str(native), *arguments], prefix)
                if prefix.with_suffix(".stderr").read_bytes():
                    raise ActiveOwnerError(f"Unexpected diagnostic: {prefix}")
                if observation.count(b"print collect-active\n") != 1:
                    raise ActiveOwnerError(
                        f"Missing/duplicate collection marker: {prefix}"
                    )
                observations.append(observation)
            if endpoints(observations[0]) != endpoints(observations[1]):
                raise ActiveOwnerError(
                    f"Recorder changed endpoints: {session.name}/{action}"
                )
            if not any(
                line.startswith(b"free ") for line in observations[1].splitlines()
            ):
                raise ActiveOwnerError(f"Missing retirements: {session.name}/{action}")
    _ = (output / "authority.json").write_text(
        json.dumps(
            {
                "native_sha256": digest(native),
                "sources": {
                    path.name: digest(path) for path in sorted(fixtures.glob("*.nut"))
                },
                "sessions": [asdict(session) for session in SESSIONS],
                "collection_actions": ["off", "on"],
                "passive_recorder": ["off", "on"],
            },
            indent=2,
        )
        + "\n"
    )
    print(
        "PASS fourteen active-owner sessions; collection actions distinct; recorder controls equal"
    )


if __name__ == "__main__":
    capture(Path(sys.argv[1]).resolve(strict=True), Path(sys.argv[2]).resolve())
