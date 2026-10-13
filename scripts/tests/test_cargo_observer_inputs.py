"""Missing or tampered observer helpers must never receive route proof credit."""

import tempfile
import unittest
from pathlib import Path

from scripts.cargo_observer_inputs import REQUIRED, verify
from scripts.gameplay_foundations import digest
from scripts.world_check_support import Json, WorldCheckError


class CargoObserverInputsTest(unittest.TestCase):
    def test_missing_helper_is_rejected(self) -> None:
        root = Path(__file__).resolve().parents[2]
        for name in REQUIRED:
            with self.subTest(name=name):
                hashes: dict[str, Json] = {
                    source: digest(root / source)
                    for source in REQUIRED
                    if source != name
                }
                try:
                    verify(root, {"sources": hashes})
                except WorldCheckError as error:
                    diagnostic = str(error)
                else:
                    raise AssertionError("Missing helper was admitted")
                assert "omits required helpers" in diagnostic

    def test_tampered_helper_bytes_are_rejected(self) -> None:
        root = Path(__file__).resolve().parents[2]
        hashes: dict[str, Json] = {source: digest(root / source) for source in REQUIRED}
        for name in REQUIRED:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                candidate = Path(temporary)
                for source in REQUIRED:
                    destination = candidate / source
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    _ = destination.write_bytes((root / source).read_bytes())
                _ = (candidate / name).write_bytes((root / name).read_bytes() + b"\n")
                try:
                    verify(candidate, {"sources": hashes})
                except WorldCheckError as error:
                    diagnostic = str(error)
                else:
                    raise AssertionError("Tampered helper was admitted")
                assert "Pinned witness source changed" in diagnostic

    def test_complete_current_helpers_are_accepted(self) -> None:
        root = Path(__file__).resolve().parents[2]
        hashes: dict[str, Json] = {source: digest(root / source) for source in REQUIRED}
        verify(root, {"sources": hashes})
