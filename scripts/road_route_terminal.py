# /// script
# requires-python = ">=3.12"
# ///
# Run: PYTHONPATH=. python3 -m scripts.road_route_terminal --help
"""Genuine ordinary terminal cycles and both-origin loading/job continuation."""

from __future__ import annotations

import argparse
import gzip
import hashlib
from dataclasses import dataclass
from pathlib import Path

from scripts.cargo_observer_inputs import current
from scripts.road_route_departure import pair
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    decode_json,
    read_json,
    write_json,
)


@dataclass(frozen=True, slots=True)
class Producer:
    oracle: Path
    runner: Path

    def compare(self, source: Path, destination: Path, ticks: int) -> None:
        current()
        pair(self.oracle, self.runner, source, destination, ticks)
        native = read_json(destination / "native-last-tick.json")
        rust = read_json(destination / "rust" / "final.runtime.json")
        if read_json(destination / "native" / "final.derived.json") != read_json(
            destination / "rust" / "final.derived.json"
        ):
            raise WorldCheckError(
                f"Terminal complete derived World differs: {destination}"
            )
        for left, right in (
            (("movement", "transport"), ("transport",)),
            (("movement", "house_caches"), ("house_caches",)),
            (("movement", "industry_caches"), ("industry_caches",)),
            (("movement", "stations", "orders"), ("orders",)),
            (("movement", "stations", "geometry"), ("station_geometry",)),
            (("movement", "stations", "infrastructure"), ("station_infrastructure",)),
            (("movement", "stations", "station_pool"), ("station_pool",)),
            (("movement", "stations", "road_stop_pool"), ("road_stop_pool",)),
        ):
            if at(native, left) != at(rust, right):
                raise WorldCheckError(
                    f"Terminal runtime differs: {left} at {destination}"
                )
        write_json(
            destination / "terminal-PASS.json",
            {
                "ticks": ticks,
                "full_saved_world": True,
                "full_derived_world": True,
                "road_caches": True,
                "house_caches": True,
                "industry_caches": True,
                "transport_pools_payment_job_results": True,
                "station_geometry_infrastructure": True,
                "order_runtime": True,
                "station_pools": True,
            },
        )
        compact(destination / "native")


def compact(directory: Path) -> None:
    """Retain every raw trace byte in verified compressed form."""
    for name in ("route.jsonl", "route-search.jsonl"):
        source = directory / name
        if not source.exists():
            continue
        raw = source.read_bytes()
        archive = source.with_name(source.name + ".gz")
        _ = archive.write_bytes(gzip.compress(raw, mtime=0))
        if gzip.decompress(archive.read_bytes()) != raw:
            raise WorldCheckError(f"Trace compression differs: {source}")
        write_json(
            source.with_name(source.name + ".sha256.json"),
            {
                "raw_bytes": len(raw),
                "raw_sha256": hashlib.sha256(raw).hexdigest(),
                "archive_sha256": hashlib.sha256(archive.read_bytes()).hexdigest(),
            },
        )
        source.unlink()


def arrivals(directory: Path, expected: list[Json]) -> None:
    trace = gzip.decompress((directory / "native" / "route.jsonl.gz").read_bytes())
    observed: list[Json] = []
    for line in trace.decode().splitlines():
        state = decode_json(line)
        if at(state, ("movement", "vehicles", 0, "order_type")) == 3:
            observed.append(
                [
                    at(state, ("runtime", "tick")),
                    at(state, ("movement", "vehicles", 0, "tile")),
                ]
            )
    if observed != expected:
        raise WorldCheckError(f"Native ordinary round trip differs: {observed}")
    write_json(directory / "arrivals-PASS.json", observed)


class Arguments(argparse.Namespace):
    oracle: Path = Path()
    runner: Path = Path()
    fixtures: Path = Path()
    output: Path = Path()


def prove(args: Arguments) -> None:
    args.output.mkdir(parents=True, exist_ok=False)
    producer = Producer(args.oracle, args.runner)
    for vehicle, boundaries, whole in (
        ("bus", (394, 508, 520, 1318), 2114),
        ("truck", (434, 555, 567, 1450), 2328),
    ):
        source = args.fixtures / (vehicle + "-valid") / "prepared.sav"
        root = args.output / vehicle
        producer.compare(source, root / "whole-cycle", whole)
        arrivals(
            root / "whole-cycle",
            [[boundaries[1], 650], [boundaries[3], 980], [whole, 650]],
        )
        for boundary in boundaries:
            segment = root / str(boundary)
            producer.compare(source, segment, boundary)
            for origin in ("native", "rust"):
                producer.compare(
                    segment / origin / "final.sav",
                    segment / ("from-" + origin),
                    whole - boundary + 1,
                )
        for ticks in (4000, 5000):
            producer.compare(source, root / str(ticks), ticks)
    write_json(
        args.output / "PASS.json",
        {
            "vehicles": ["bus", "truck"],
            "whole_cycles": [2114, 2328],
            "save_origins": ["native", "rust"],
        },
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("oracle", "runner", "fixtures", "output"):
        _ = parser.add_argument("--" + name, type=Path, required=True)
    prove(parser.parse_args(namespace=Arguments()))


if __name__ == "__main__":
    main()
