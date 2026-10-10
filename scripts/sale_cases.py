"""Road sale command sequences over original-cleared or original-purchased inputs."""

from __future__ import annotations

from scripts.world_check_support import Json, WorldCheckError, at


def buy(tile: Json, engine: Json) -> Json:
    return {
        "kind": "build_vehicle",
        "tile": tile,
        "engine": engine,
        "cargo": 255,
        "use_free_vehicles": False,
        "client_id": 0,
    }


def sell(tile: Json, vehicle: Json, chain: bool = False) -> Json:
    return {
        "kind": "sell_vehicle",
        "location": tile,
        "vehicle": vehicle,
        "sell_chain": chain,
        "backup_order": False,
        "client_id": 0,
    }


def commands(requests: list[tuple[Json, str]]) -> Json:
    return {
        "schema_version": 1,
        "actions": [
            {
                "ordinal": index,
                "op": "command",
                "request": {"company": 0, "mode": mode, "command": command},
            }
            for index, (command, mode) in enumerate(requests)
        ],
    }


def plan(case: Json) -> Json:
    tile = at(case, ("tile",))
    engines = at(case, ("engines",))
    name = at(case, ("name",))
    match engines:
        case list() if engines:
            engine = engines[0]
        case _:
            raise WorldCheckError("Missing sale engines")
    requests: list[tuple[Json, str]] = []
    match name:
        case (
            "temperate-original"
            | "temperate-realistic"
            | "arctic"
            | "tropic"
            | "toyland"
        ):
            for item in engines:
                requests.extend(
                    [
                        (buy(tile, item), "post"),
                        (sell(tile, 0), "estimate"),
                        (sell(tile, 0, True), "post"),
                    ]
                )
        case "reuse":
            requests.extend([(buy(tile, engine), "post") for _ in range(3)])
            requests.extend(
                [
                    (sell(tile, 1), "post"),
                    (buy(tile, engine), "post"),
                    (sell(tile, 0), "post"),
                    (sell(tile, 2), "post"),
                    (sell(tile, 1), "post"),
                    (buy(tile, engine), "post"),
                ]
            )
        case "legacy":
            requests.extend(
                [
                    (sell(tile, 0), "estimate"),
                    (sell(tile, 0), "post"),
                    (buy(tile, engine), "post"),
                    (sell(tile, 0), "post"),
                    (buy(tile, engine), "post"),
                ]
            )
        case "named-survivor":
            requests.extend(
                [
                    (sell(tile, 0), "estimate"),
                    (sell(tile, 0), "post"),
                    (buy(tile, engine), "post"),
                    (sell(tile, 0), "post"),
                ]
            )
        case "missing":
            requests.extend(
                [(sell(tile, 1048575), "estimate"), (sell(tile, 1048575), "post")]
            )
        case "zero-location" | "invalid-location":
            location = 0 if name == "zero-location" else 4294967295
            requests.extend(
                [(sell(location, 0), "estimate"), (sell(location, 0), "post")]
            )
        case (
            "crashed"
            | "moving"
            | "unstopped"
            | "wrong-state"
            | "outside-depot"
            | "nonowner"
            | "pause"
            | "negative-value"
            | "minimum-value"
            | "zero-value"
            | "insufficient"
        ):
            requests.extend([(sell(tile, 0), "estimate"), (sell(tile, 0), "post")])
        case _:
            raise WorldCheckError("Unregistered sale case")
    return commands(requests)
