# SPDX-License-Identifier: GPL-2.0-only
"""Original-command house fixtures and complete-loop checkpoint schedules."""

from __future__ import annotations

from pathlib import Path

from scripts.house_native import HouseNative, command
from scripts.world_check_support import Json


def checkpoints(count: int) -> list[Json]:
    points = sorted({0, 1, 255, 256, 1792, 2048, 4096, 5888, 5944, 6144, count})
    actions: list[Json] = []
    previous = 0
    for point in points:
        if point > count:
            continue
        actions.extend(
            [
                {"op": "tick", "count": point - previous},
                {"op": "checkpoint", "label": f"tick_{point}"},
            ],
        )
        previous = point
    return actions


def prepare(native: HouseNative, ready: Path, output: Path) -> None:
    multi = native.run(
        output / "prepare-multi",
        [command("house_place", tile=1060, house=20, replace=False)],
        source=ready,
    )
    bitcount = native.run(
        output / "prepare-bitcount",
        [command("house_setting", name="economy.town_cargogen_mode", value=1)],
        source=multi,
    )
    scale = native.run(
        output / "prepare-scale15",
        [command("house_setting", name="economy.town_cargo_scale", value=15)],
        source=bitcount,
    )
    paused = native.run(
        output / "prepare-paused",
        [command("pause", company=0, mode=0, paused=True)],
        source=ready,
    )
    for name, source, count in [
        ("original-6144", ready, 6144),
        ("multi-6144", multi, 6144),
        ("bitcount-8192", bitcount, 8192),
        ("scale15-8192", scale, 8192),
        ("original-year", ready, 27010),
        ("paused", paused, 7),
        ("zero", ready, 0),
    ]:
        _ = native.run(output / name, checkpoints(count), source=source)
    animation = native.run(
        output / "prepare-animation",
        [
            command("house_setting", name="economy.place_houses", value=2),
            command("house_place", tile=1060, house=4, replace=False),
        ],
        source=ready,
    )
    growth = native.run(
        output / "prepare-growth",
        [command("house_growth_rate", town=0, rate=1)],
        source=ready,
    )
    for name, source in [("animation-256", animation), ("growth-256", growth)]:
        _ = native.run(output / name, [{"op": "tick", "count": 256}], source=source)
    _ = native.run(
        output / "observer-off-6144",
        [{"op": "tick", "count": 6144}],
        source=ready,
        observe=False,
    )
