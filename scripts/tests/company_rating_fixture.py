"""Synthetic shape fixture; never presented as original gameplay evidence."""

from collections.abc import Generator
from contextlib import contextmanager

from scripts.movement_ci.value import EvidenceError, Json


def economy() -> Json:
    result: Json = {
        "income": 0,
        "expenses": 0,
        "delivered_cargo": list[Json]([0] * 64),
        "performance_history": 0,
        "company_value": 0,
    }
    return result


def snapshot() -> dict[str, Json]:
    result: dict[str, Json] = {
        "schema_version": 1,
        "economy_month": 1,
        "scores": [
            {"owner": owner, "parts": list[Json]([0] * 10)} for owner in range(15)
        ],
        "score_info": [
            {"id": index, "needed": needed, "weight": weight}
            for index, (needed, weight) in enumerate(
                (
                    (120, 100),
                    (80, 100),
                    (10000, 100),
                    (50000, 50),
                    (100000, 100),
                    (40000, 400),
                    (8, 50),
                    (10000000, 50),
                    (250000, 50),
                    (0, 0),
                )
            )
        ],
        "companies": [],
        "runtime": {
            "schema_version": 1,
            "companies": [],
            "groups": [],
            "vehicles": [],
            "news_queue": 0,
            "context": {
                "current_company": 16,
                "local_company": 0,
                "networking": False,
                "network_server": False,
                "vehicle_income_warn": True,
                "old_vehicle_warn": True,
            },
            "clock": {
                "tick": 1,
                "economy_date": 0,
                "economy_fract": 0,
                "economy_year": 0,
                "calendar_date": 0,
                "calendar_fract": 0,
            },
        },
    }
    return result


@contextmanager
def rejects() -> Generator[None]:
    try:
        yield
    except EvidenceError:
        return
    raise AssertionError("malformed native rating observation accepted")


def owned() -> dict[str, Json]:
    result = snapshot()
    result["economy_month"] = 3
    result["companies"] = [
        {
            "id": 0,
            "valid_history": 1,
            "block_preview": 0,
            "money": 0,
            "loan": 0,
            "hq_tile": 2**32 - 1,
            "current": economy(),
            "history": [economy() for _ in range(24)],
        }
    ]
    statistics: Json = {
        "profit_last_year": 0,
        "profit_last_year_min_age": 0,
        "num_vehicle_min_age": 0,
        "num_vehicle": 0,
        "num_engines": [],
        "autoreplace_defined": False,
        "autoreplace_finished": False,
    }
    runtime = result["runtime"]
    if not isinstance(runtime, dict):
        raise TypeError("fixture runtime object required")
    runtime["companies"] = [
        {
            "id": 0,
            "is_ai": False,
            "all": list[Json]([statistics] * 4),
            "default": list[Json]([statistics] * 4),
            "yearly_expenses": [list[Json]([0] * 13) for _ in range(3)],
        }
    ]
    return result
