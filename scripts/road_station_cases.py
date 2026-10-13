"""Native first-allocation and facility-list construction cases."""

from __future__ import annotations

from dataclasses import dataclass, replace

from scripts.world_check_support import Json


@dataclass(frozen=True, slots=True)
class StationCase:
    tile: int = 650
    stop_type: int = 0
    direction: int = 0
    station: int = 65533
    company: int = 0
    estimate: bool = False
    width: int = 1
    length: int = 1
    road_type: int = 0
    spec_index: int = 0

    def request(self) -> Json:
        return {
            "company": self.company,
            "mode": "estimate" if self.estimate else "post",
            "command": {
                "kind": "build_road_stop",
                "tile": self.tile,
                "stop": {
                    "width": self.width,
                    "length": self.length,
                    "stop_type": self.stop_type,
                    "drive_through": False,
                    "direction": self.direction,
                    "road_type": self.road_type,
                    "spec_class": 0,
                    "spec_index": self.spec_index,
                    "station_to_join": self.station,
                    "adjacent": True,
                },
            },
        }


def descriptor(cases: tuple[StationCase, ...], label: str = "road-stations") -> Json:
    actions: list[Json] = []
    for index, case in enumerate(cases):
        actions.extend(
            (
                {"op": "command", "request": case.request()},
                {"op": "save", "label": f"step-{index}"},
            )
        )
    return {
        "schema_version": 1,
        "case": label,
        "role": "sp",
        "exit": True,
        "actions": actions,
    }


def portfolio() -> Json:
    first = StationCase()
    cases = tuple(
        replace(first, tile=650 + direction, direction=direction)
        for direction in range(4)
    )
    cases += tuple(
        replace(first, tile=666 + direction, direction=direction, stop_type=1)
        for direction in range(4)
    )
    cases += tuple(
        replace(
            first,
            tile=714 + direction,
            direction=direction,
            stop_type=direction // 2,
            station=0,
        )
        for direction in range(4)
    )
    cases += (
        replace(first, tile=720, station=0, estimate=True),
        replace(first, tile=1000, station=0),
        replace(first, tile=730, station=60000),
        replace(first, tile=730, station=65534),
        replace(first, tile=730, direction=4),
        replace(first, tile=730, road_type=63),
        replace(first, tile=730, spec_index=1),
        replace(first, tile=730, width=0),
        replace(first, tile=730, width=13),
        replace(first, tile=730, stop_type=2),
    )
    return descriptor(cases)
