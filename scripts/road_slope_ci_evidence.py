# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run through scripts/check-road-slope-ci.py.
from __future__ import annotations

import tempfile
from itertools import product
from pathlib import Path
from typing import Final

from scripts.context_ci_support import exact, process, verify_identity
from scripts.depot_build_archive import bounded_paths
from scripts.depot_build_provenance import snapshot
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import sequence, text
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    read_json,
    replace,
    write_json,
)

SLOPE_TEST: Final = "commands::road::slope::native::original_road_slope_matrix"


def compare_probe(probe: Json, output: Path) -> None:
    rows = sequence(at(probe, ("rows",)))
    if len(rows) != 155648 or at(probe, ("schema_version",)) != 1:
        raise WorldCheckError("Incomplete native road slope matrix")
    tuples = product(
        (*range(15), 23, 27, 29, 30), range(16), range(16), range(16), (False, True)
    )
    for observed, values in zip(rows, tuples, strict=True):
        wanted: list[Json] = list(values)
        actual = [
            at(observed, (key,))
            for key in ("slope", "requested", "existing", "other", "enabled")
        ]
        if not exact(actual, wanted):
            raise WorldCheckError("Native road slope tuple membership differs")
    compare_prices(probe)
    compare_restoration(probe, output)


def compare_prices(probe: Json) -> None:
    prices = sequence(at(probe, ("price_probes",)))
    if len(prices) != 5:
        raise WorldCheckError("Missing native road slope price probes")
    for row, price in zip(prices, (0, 1, -1, -(2**63), 2**63 - 1), strict=True):
        if not exact(at(row, ("price",)), price):
            raise WorldCheckError("Native price probe identity differs")
        for name, value in (
            ("slope", 1),
            ("requested", 1),
            ("existing", 0),
            ("other", 0),
            ("enabled", True),
        ):
            if not exact(at(row, ("result", name)), value):
                raise WorldCheckError("Native price probe tuple differs")


def compare_restoration(probe: Json, output: Path) -> None:
    if not exact(at(probe, ("before",)), at(probe, ("after",))) or not exact(
        at(probe, ("foundation_price",)), at(probe, ("before", "foundation_price"))
    ):
        raise WorldCheckError("Native slope settings/price restoration differs")
    live = at(probe, ("live_before",))
    if not exact(live, at(probe, ("live_after",))):
        raise WorldCheckError("Native slope live restoration differs")
    for name in ("canonical", "vectors", "reload"):
        observed = read_json(output / "case" / name / "depot-runtime.json")
        if not exact(
            live,
            {
                "depot": at(observed, ("runtime",)),
                "vehicles": at(observed, ("vehicles",)),
            },
        ):
            raise WorldCheckError(
                "Independent slope canonical/reload live state differs"
            )


def controls(output: Path, binary: Path, probe: Json) -> None:
    for name, replacement in (
        ("pieces", 15),
        ("success", True),
        ("cost", 1),
        ("expenses", 0),
    ):
        changed = output / "corruption" / name
        rejection = output / "rejection" / name
        argv: list[Json] = [
            "env",
            f"ROAD_SLOPE_CASE={changed}",
            str(binary),
            "--ignored",
            "--exact",
            SLOPE_TEST,
        ]
        if not exact(read_json(rejection / "argv.json"), argv) or not exact(
            read_json(rejection / "process.json"), {"returncode": 101, "expected": 101}
        ):
            raise WorldCheckError("Slope rejection executable/status differs")
        log = (rejection / "stdout.log").read_text()
        if (
            "native road slope output differs" not in log
            or "1 failed; 0 ignored" not in log
            or (changed / "comparison.json").exists()
        ):
            raise WorldCheckError(
                "Slope mutation did not reach the actual rejected comparison"
            )
        value = read_json(changed / "vectors/road-slope.json")

        original = at(probe, ("rows", 0, name))
        if exact(original, replacement) or not exact(
            at(value, ("rows", 0, name)), replacement
        ):
            raise WorldCheckError("Missing actual native slope mutation")
        replace(value, ("rows", 0, name), original)
        if not exact(value, probe):
            raise WorldCheckError("Slope control changed more than its named output")


def artifacts(root: Path, output: Path, layout: Json) -> None:
    expected = {text(value) for value in sequence(at(layout, ("paths",)))}
    manifest = read_json(output / "manifest.json")
    match manifest:
        case dict() if set(manifest) == expected:
            for name, sha in manifest.items():
                if digest(output / name) != sha:
                    raise WorldCheckError("Original slope artifact changed")
        case _:
            raise WorldCheckError("Slope evidence path identities differ")
    _ = bounded_paths(
        output,
        expected
        | {
            "manifest.json",
            "summary.json",
            *[
                f"ci-invocation/{name}"
                for name in (
                    "stdout.log",
                    "stderr.log",
                    "argv.json",
                    "environment.json",
                    "process.json",
                )
            ],
        },
    )
    verify_identity(root, output, occupancy=True)
    provenance = read_json(output / "provenance.json")
    oracle = Path(text(at(provenance, ("oracle",))))
    with tempfile.TemporaryDirectory() as scratch:
        snapshot(root, Path(scratch), oracle)
        current = read_json(Path(scratch) / "provenance.json")
        if not exact(at(current, ("sources",)), at(provenance, ("sources",))):
            raise WorldCheckError("Slope dynamic source inventory incomplete")
    inputs = read_json(output / "inputs-before.json")
    match inputs:
        case dict() if set(inputs) == {
            text(v) for v in sequence(at(layout, ("inputs",)))
        }:
            for name, sha in inputs.items():
                if digest(output / name) != sha:
                    raise WorldCheckError("Slope input changed during proof")
        case _:
            raise WorldCheckError("Slope input inventory incomplete")


def validate_slope(root: Path, output: Path, layout: Json) -> None:
    artifacts(root, output, layout)
    identities = read_json(output / "test-binaries.json")
    for name in ("prepare", "case/canonical", "case/vectors", "case/reload"):
        if not exact(
            read_json(output / "logs" / name / "process.json"), {"returncode": 0}
        ):
            raise WorldCheckError("Original slope observation process failed")
    if not exact(
        read_json(output / "original-fleet/native-command/process.json"),
        {"returncode": 0, "expected": 0},
    ):
        raise WorldCheckError("Original slope fleet preparation failed")
    if not exact(
        read_json(output / "summary.json"),
        {
            "passed": True,
            "vectors": 155648,
            "price_probes": 5,
            "corruption_rejections": 4,
            "pure_helper_only": True,
        },
    ):
        raise WorldCheckError("Slope raw driver coverage differs")
    binary = Path(text(at(identities, ("ottd_sim", "retained"))))
    process(output / "case/logs" / SLOPE_TEST, binary, SLOPE_TEST)
    process(
        output / "logs/prepare_fleet_source",
        Path(text(at(identities, ("native_depot_build", "retained")))),
        "prepare_fleet_source",
    )
    if not exact(
        read_json(output / "case/comparison.json"),
        {
            "rows": 155648,
            "price_probes": 5,
            "full_saved_live_equal": True,
            "settings_restored": True,
        },
    ):
        raise WorldCheckError("Slope comparison receipt incomplete")
    probe = read_json(output / "case/vectors/road-slope.json")
    compare_probe(probe, output)
    controls(output, binary, probe)
    write_json(
        output / "coverage.json",
        {
            "vectors": 155648,
            "price_probes": 5,
            "mutations_rejected": 4,
            "pure_test_invocations": 1,
            "input_preparations": 1,
            "complete_restoration_assertions": True,
        },
    )
