from __future__ import annotations

import copy
import shutil
from pathlib import Path

from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import sequence, text
from scripts.language_ci_compare import compare, mapping
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json

CONTROLS = (
    "change-flags",
    "change-test-rating",
    "drop-scope",
    "duplicate-exec-penalty",
    "missing-failed-step",
    "modify-pass0",
    "reset-nested-map",
    "set-test-have-rating",
    "swap-tree-order",
)


def integer(value: Json) -> int:
    match value:
        case bool():
            raise WorldCheckError("Boolean tree event integer")
        case int() as result:
            return result
        case _:
            raise WorldCheckError("Non-integer tree event field")


def first(events: list[Json], kind: str, *, testing: bool | None = None) -> int:
    for index, event in enumerate(events):
        if at(event, ("kind",)) == kind and (
            testing is None or at(event, ("test_mode",)) is testing
        ):
            return index
    raise WorldCheckError("Missing actual mutation target")


def reset_nested(events: list[Json]) -> None:
    choices = [
        row
        for row in events
        if at(row, ("kind",)) == "scope_enter"
        and integer(at(row, ("depth",))) > 1
        and (integer(at(row, ("map_entries",))) > 0)
    ]
    if not choices:
        raise WorldCheckError("Missing nested map mutation target")
    mapping(choices[0])["map_entries"] = 0


def swap_trees(events: list[Json]) -> None:
    trees = [mapping(row) for row in events if at(row, ("kind",)) == "tree"]
    if len(trees) < 2:
        raise WorldCheckError("Missing tree order mutation targets")
    trees[0]["tile"], trees[1]["tile"] = (trees[1]["tile"], trees[0]["tile"])


def mutate_events(events: list[Json], name: str) -> list[Json]:
    match name:
        case "change-flags":
            item = mapping(events[first(events, "tree")])
            item["flags"] = integer(item["flags"]) + 1
        case "change-test-rating":
            item = mapping(events[first(events, "applied", testing=True)])
            item["after_rating"] = integer(item["after_rating"]) - 1
        case "drop-scope":
            del events[first(events, "scope_leave")]
        case "duplicate-exec-penalty":
            index = first(events, "applied", testing=False)
            item = dict(mapping(events[index]))
            item["before_rating"] = item["after_rating"]
            item["after_rating"] = integer(item["after_rating"]) - 35
            item["saved_rating"] = item["after_rating"]
            events.insert(index + 1, item)
        case "missing-failed-step":
            events = [event for event in events if at(event, ("kind",)) != "applied"]
        case "modify-pass0":
            mapping(events[first(events, "suppressed")]).update(
                {
                    "kind": "applied",
                    "before_rating": 500,
                    "after_rating": 465,
                    "saved_rating": 500,
                    "have_ratings": 0,
                }
            )
        case "reset-nested-map":
            reset_nested(events)
        case "set-test-have-rating":
            mapping(events[first(events, "applied", testing=True)])["have_ratings"] = 1
        case "swap-tree-order":
            swap_trees(events)
        case _:
            raise WorldCheckError("Unknown tree corruption")
    return events


def mutate(original: Json, name: str) -> Json:
    events = copy.deepcopy(sequence(original))

    events = mutate_events(events, name)
    for ordinal, event in enumerate(events):
        mapping(event)["ordinal"] = ordinal
    if events == original:
        raise WorldCheckError("Ineffective tree corruption")
    return events


def prepare(output: Path) -> None:
    for name in CONTROLS:
        baseline = (
            output
            / "prior/runs"
            / ("terraform-limit" if name == "missing-failed-step" else "terraform-four")
        )
        directory = output / "corpus/controls" / name
        directory.mkdir(parents=True)
        for source in baseline.iterdir():
            if source.is_file():
                _ = shutil.copy2(source, directory / source.name)
        native = read_json(directory / "results.json")
        trace = mapping(at(native, ("actions", 0, "native_metadata", "tree_rating")))
        trace["events"] = mutate(trace["events"], name)
        write_json(directory / "results.json", native)
        _ = (directory / "baseline.txt").write_text(str(baseline) + "\n")
        write_json(
            directory / "mutation.json",
            {
                "name": name,
                "baseline_sha256": digest(baseline / "results.json"),
                "mutated_sha256": digest(directory / "results.json"),
            },
        )


def validate(output: Path) -> None:
    observed = output / "tests/corruption_controls-0/observations"
    expected: list[str] = []
    for name in CONTROLS:
        rejection = f"control-{name}-rejection.json"
        expected.append(rejection)
        row = read_json(observed / rejection)
        compare(at(row, ("control",)), name)
        if not text(at(row, ("error",))).endswith(": trace mismatch"):
            raise WorldCheckError(
                "Tree corruption failed before actual trace comparison"
            )
        stages: list[Json] = []
        for stage in ("baseline", "mutated", "restored"):
            paths = list(observed.glob(f"control-{name}-{stage}-*.json"))
            if len(paths) != 1:
                raise WorldCheckError("Missing actual tree corruption execution")
            expected.append(paths[0].name)
            stages.append(read_json(paths[0]))
        compare(stages[0], stages[1])
        compare(stages[0], stages[2])
    compare([*sorted(p.name for p in observed.iterdir())], [*sorted(expected)])
