# SPDX-License-Identifier: GPL-2.0-only
"""Compare the original empty industry's live manager with the published world."""

from pathlib import Path

from scripts.grf_control_evidence import sequence
from scripts.language_ci_compare import number
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


def manager_cache(world: Json, observed: Json) -> None:
    records = at(world, ("chunks", "ITBL", "records"))
    if not isinstance(records, dict) or set(records) != {str(i) for i in range(240)}:
        raise WorldCheckError("Incomplete saved industry type membership")
    rows = [records[str(i)] for i in range(240)]
    wanted = at(world, ("chunks", "IBLD", "records", "0", "wanted_inds"))
    if at(observed, ("builder",)) != {"types": rows, "wanted_inds": wanted}:
        raise WorldCheckError(
            "Native live industry builder differs from published world"
        )
    if at(world, ("chunks", "INDY", "records")) != {}:
        raise WorldCheckError("This manager proof requires the empty industry pool")
    if at(observed, ("industries",)) != [] or at(observed, ("type_members",)) != [
        [] for _ in range(240)
    ]:
        raise WorldCheckError("Native industry pool/type membership is not empty")
    width = number(at(world, ("chunks", "MAPS", "records", "0", "dim_x")))
    height = number(at(world, ("chunks", "MAPS", "records", "0", "dim_y")))
    if at(observed, ("daily_increment",)) != width * height // 31:
        raise WorldCheckError("Native map-scaled industry increment differs")


def boundaries(native: Path, output: Path) -> None:
    protocol = read_json(native / "protocol.json")
    labels = ["initial", "final"]
    for action in sequence(at(protocol, ("actions",))):
        if at(action, ("op",)) == "checkpoint":
            label = at(action, ("label",))
            if not isinstance(label, str):
                raise WorldCheckError("Industry checkpoint label is not text")
            labels.append(label)
    for label in labels:
        manager_cache(
            read_json(output / f"{label}.world.json"),
            read_json(native / f"{label}.industries.json"),
        )
    write_json(
        output / "manager-PASS.json",
        {"complete_builder_type_pool_increment_boundaries": len(labels)},
    )
