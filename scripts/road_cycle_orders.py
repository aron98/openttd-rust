"""Focused original road-order command portfolio; invoked from the private lane."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path

from scripts.replay_matrix import ReplayMatrix
from scripts.service_replay_cases import check_native_actions
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


@dataclass(frozen=True, slots=True)
class OrderCase:
    vehicle: int = 0
    destination: int = 0
    index: int = 0
    kind: int = 33
    flags: int = 0
    refit: int = 254
    estimate: bool = False
    location: int = 0

    def request(self) -> Json:
        return {
            "company": 0,
            "mode": "estimate" if self.estimate else "post",
            "command": {
                "kind": "insert_order",
                "location": self.location,
                "vehicle": self.vehicle,
                "index": self.index,
                "order": {
                    "type": self.kind,
                    "flags": self.flags,
                    "dest": self.destination,
                    "refit_cargo": self.refit,
                    "wait_time": 0,
                    "travel_time": 0,
                    "max_speed": 65535,
                },
            },
        }


def portfolio() -> Json:
    cases = (
        OrderCase(),
        OrderCase(destination=1, index=1),
        OrderCase(destination=2),
        OrderCase(vehicle=1, destination=2),
        OrderCase(vehicle=1),
        OrderCase(destination=65535),
        OrderCase(kind=2, flags=2),
        OrderCase(kind=2, flags=18),
        OrderCase(flags=5),
        OrderCase(refit=0),
        OrderCase(index=255),
        OrderCase(estimate=True),
        OrderCase(index=1, location=670),
        OrderCase(flags=32),
        OrderCase(kind=2, flags=34),
        OrderCase(vehicle=64000),
    )
    actions: list[Json] = []
    for index, case in enumerate(cases):
        actions.extend(
            (
                {"ordinal": index * 2, "op": "command", "request": case.request()},
                {
                    "ordinal": index * 2 + 1,
                    "op": "checkpoint",
                    "label": f"step-{index}",
                },
            )
        )
    return {"schema_version": 1, "actions": actions}


def capture(matrix: ReplayMatrix, source: Path) -> None:
    """Execute every command through the original and Rust CLI and compare all views."""
    matrix.artifacts.mkdir(parents=True, exist_ok=False)
    descriptor = matrix.artifacts / "actions.json"
    write_json(descriptor, portfolio())
    native = matrix.native(source, descriptor, matrix.artifacts)
    rust = matrix.rust(source, descriptor, matrix.artifacts)
    results = read_json(native / "results.json")
    check_native_actions(results)
    outcomes = (
        True,
        True,
        False,
        True,
        False,
        False,
        True,
        True,
        False,
        False,
        False,
        True,
        True,
        True,
        False,
        False,
    )
    for index in range(32):
        action = at(results, ("actions", index))
        if at(action, ("before", "random")) != at(action, ("after", "random")):
            raise WorldCheckError("Order insertion changed gameplay RNG")
        if (
            index % 2 == 0
            and at(action, ("receipt", "result", "success")) != outcomes[index // 2]
        ):
            raise WorldCheckError(
                "Native command outcome differs from portfolio expectation"
            )
    matrix.compare_outputs(native, rust, matrix.artifacts / "comparison")
    write_json(
        matrix.artifacts / "PASS.json",
        {"commands": 16, "world_runtime_receipt_rng": True},
    )


if __name__ == "__main__":
    root = Path(__file__).resolve().parents[1]
    evidence = root / ".omo/evidence/road-cycle"
    capture(
        ReplayMatrix(
            root.parent / "stage04-road-cycle-target/debug/ottd",
            root / ".reference/snapshot-build/openttd",
            root / ".reference/snapshot-build/openttd",
            evidence / "orders-final",
        ),
        evidence / "prepared-02/final.sav",
    )
