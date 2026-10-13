# SPDX-License-Identifier: GPL-2.0-only
"""Original-command forest policy witnesses; no saved-state field assignment."""

from pathlib import Path

from scripts.grf_control_evidence import sequence
from scripts.house_native import command
from scripts.industry_native import IndustryNative
from scripts.language_ci_compare import number
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    decode_json,
    read_json,
    write_json,
)


def draw(state: tuple[int, int]) -> tuple[tuple[int, int], int]:
    """Pinned core/random_func.cpp Next, used only to identify the native branch."""
    first, second = state
    rotated = ((first >> 3) | (first << 29)) & 0xFFFFFFFF
    value = (rotated - 1) & 0xFFFFFFFF
    mixed = second ^ 0x1234567F
    other = ((mixed >> 7) | (mixed << 25)) & 0xFFFFFFFF
    return ((first + other + 1) & 0xFFFFFFFF, value), value


def native_halving(directory: Path) -> bool:
    events = [
        decode_json(line)
        for line in (directory / "industry-events.jsonl").read_text().splitlines()
    ]
    daily = [event for event in events if at(event, ("phase",)) == "daily"]
    if len(daily) != 2:
        raise WorldCheckError("Policy search did not reach one natural industry day")
    words = sequence(at(daily[0], ("random",)))
    state = (number(words[0]), number(words[1]))
    state, builder = draw(state)
    state, _selection = draw(state)
    state, change = draw(state)
    state, increase = draw(state)
    expected = at(daily[1], ("random",))
    return (
        list(state) == expected
        and ((builder & 0xFFFF) * 100 + 50) >> 16 >= 3
        and ((change & 0xFFFF) * 3 + 1) >> 16 < 1
        and ((increase & 0xFFFF) * 3 + 1) >> 16 >= 1
    )


def search_halving(
    binary: Path, source: Path, output: Path, level: int, start_tile: int
) -> Path:
    for count in range(1, 65):
        prepared = IndustryNative(binary, source).run(
            output / f"plant-{count}",
            [
                command(
                    "tree_growth_plant",
                    company=0,
                    start_tile=start_tile,
                    tile=start_tile + count - 1,
                    type=255,
                )
            ],
        )
        trace = output / f"tick-{count}"
        final = IndustryNative(binary, prepared).run(
            trace, [{"op": "tick", "count": 1}], whole_loop=True
        )
        world = read_json(trace / "final.world.json")
        actual = at(world, ("chunks", "INDY", "records", "0", "prod_level"))
        if actual == level and native_halving(trace):
            write_json(
                output / "selected.json",
                {"count": count, "trace": str(trace), "level": level},
            )
            return final
    raise WorldCheckError("No genuine native halving branch within command bound")


def prepare_protection(binary: Path, source: Path, output: Path) -> Path:
    output.mkdir()
    actions: list[Json] = [
        command("industry_build", tile=1032, type=3, layout=0, fund=True, seed=12345),
        command("tree_growth_plant", company=0, start_tile=1300, tile=1300, type=255),
    ]
    built = IndustryNative(binary, source).run(output / "build", actions)
    prefix = IndustryNative(binary, built).run(
        output / "first-prefix", [{"op": "tick", "count": 9249}], whole_loop=True
    )
    for ordinal, (level, start) in enumerate([(8, 1428), (4, 1556), (4, 1684)]):
        if ordinal:
            prefix = IndustryNative(binary, prefix).run(
                output / f"prefix-{ordinal}",
                [{"op": "tick", "count": 9175}],
                whole_loop=True,
            )
        prefix = search_halving(
            binary, prefix, output / f"halving-{ordinal}", level, start
        )
    write_json(
        output / "PASS.json",
        {"natural_days": [125, 249, 373], "levels": [8, 4, 4], "save": str(prefix)},
    )
    return prefix
