"""Full state and opt-in original sale phase comparison."""

from __future__ import annotations

from functools import partial
from pathlib import Path

from scripts.purchase_replay import rust
from scripts.replay_matrix import ReplayMatrix
from scripts.sale_saved_state import compare_saved
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


def compare_live(matrix: ReplayMatrix, native: Path, actual: Path, case: Path) -> None:
    actions = at(read_json(native / "results.json"), ("actions",))
    rust_live = read_json(actual / "sale-live.json")
    match actions:
        case list():
            pass
        case _:
            raise WorldCheckError("Missing native sale actions")
    expected: list[Json] = []
    phases: list[Json] = []
    actual_phases: list[Json] = []
    for action in actions:
        if at(action, ("op",)) != "command":
            continue
        metadata = at(action, ("native_metadata",))
        before = at(metadata, ("sale_before",))
        after = before
        match metadata:
            case dict():
                for phase in ("test", "exec", "result"):
                    if phase in metadata:
                        value = at(metadata, (phase, "sale"))
                        if phase != "test":
                            after = value
                        phases.append(
                            {
                                "ordinal": at(action, ("ordinal",)),
                                "phase": phase,
                                "live": value,
                            }
                        )
                        actual_phases.append(
                            {
                                "ordinal": at(action, ("ordinal",)),
                                "phase": phase,
                                "live": at(
                                    rust_live,
                                    (
                                        len(expected),
                                        "before" if phase == "test" else "after",
                                    ),
                                ),
                            }
                        )
            case _:
                raise WorldCheckError("Missing native sale phase metadata")
        expected.append(
            {"ordinal": at(action, ("ordinal",)), "before": before, "after": after}
        )
    write_json(case / "native-sale-live.json", expected)
    write_json(case / "native-sale-phases.json", phases)
    write_json(case / "rust-sale-phases.json", actual_phases)
    matrix.compare(
        case / "native-sale-live.json", actual / "sale-live.json", case / "live-compare"
    )
    matrix.compare(
        case / "native-sale-phases.json",
        case / "rust-sale-phases.json",
        case / "phase-compare",
    )


def scenario(matrix: ReplayMatrix, source: Path, plan: Path, case: Path) -> None:
    case.mkdir(parents=True, exist_ok=True)
    empty = case / "empty.json"
    write_json(empty, {"schema_version": 1, "actions": []})
    loaded = matrix.native(source, empty, case / "load")
    native = matrix.native(loaded / "final.sav", plan, case)
    actual = rust(loaded / "final.sav", plan, case)
    for extension in ("world.json", "derived.json"):
        matrix.compare(
            loaded / f"final.{extension}",
            native / f"initial.{extension}",
            case / f"canonical-{extension}",
        )
    matrix.compare_outputs(
        native,
        actual,
        case / "compare",
        saved_world_compare=partial(compare_saved, plan, native, actual),
    )
    compare_live(matrix, native, actual, case / "compare")
    reloaded = matrix.native(actual / "final.sav", empty, case / "reload")
    for extension in ("world.json", "derived.json"):
        matrix.compare(
            actual / f"final.{extension}",
            reloaded / f"initial.{extension}",
            case / f"reload-{extension}",
        )
