from __future__ import annotations

import copy
import shutil
from pathlib import Path

from scripts.context_ci_support import sources
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import sequence
from scripts.language_ci_compare import compare, mapping
from scripts.tree_ci_evidence import membership
from scripts.tree_ci_run import TreeRun
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


def source_probe(run: TreeRun, expected: Json) -> None:
    name = "crates/ottd-sim/src/commands/town_rating.rs"
    directory = run.job.output / "controls/wrong-source"
    directory.mkdir(parents=True)
    original = run.job.root / name
    changed = directory / name
    changed.parent.mkdir(parents=True)
    _ = shutil.copy2(original, changed)
    before = digest(changed)
    _ = changed.write_bytes(changed.read_bytes() + b"\n")
    try:
        sources(directory, {"sources": {name: at(expected, (name,))}})
    except WorldCheckError:
        write_json(
            directory / "rejected.json",
            {
                "source": name,
                "before": before,
                "after": digest(changed),
                "rejected": True,
            },
        )
    else:
        raise WorldCheckError("Physical wrong source was admitted")
    compare(digest(original), before)


def subset_probe(run: TreeRun, lib: Path, selector: str) -> None:
    directory = run.job.output / "controls/subset"
    (directory / "draft").mkdir(parents=True)
    manifest = read_json(run.job.output / "corpus/draft/native-cases.json")
    changed = copy.deepcopy(mapping(manifest))
    cases = changed["cases"]
    if not isinstance(cases, list):
        raise WorldCheckError("Missing actual case list")
    candidates = [
        index
        for index, row in enumerate(cases)
        if at(row, ("domain",)) == "terrain_parity"
    ]
    if len(candidates) != 45:
        raise WorldCheckError("Subset control baseline is incomplete")
    removed = cases.pop(candidates[0])
    write_json(directory / "draft/native-cases.json", changed)
    result = run.job.run(
        "subset-test",
        [str(lib), "--exact", selector, "--ignored", "--nocapture"],
        {
            "TREE_NATIVE_CORPUS": str(directory),
            "TREE_CORPUS_OUTPUT": str(directory / "observations"),
        },
        expected=101,
    )
    if (
        result.stdout.count("NATIVE_CORPUS_CASE ") != 44
        or "left: 44" not in result.stderr
        or "right: 45" not in result.stderr
    ):
        raise WorldCheckError(
            "Subset rejection was not the actual fixed-case cardinality failure"
        )
    try:
        membership(
            [str(at(row, ("id",))) for row in cases],
            [str(at(row, ("id",))) for row in sequence(at(manifest, ("cases",)))],
        )
    except WorldCheckError:
        write_json(
            directory / "rejected.json",
            {
                "removed": removed,
                "actual_rust_cases": 44,
                "rust_exit": 101,
                "membership_rejected": True,
            },
        )
    else:
        raise WorldCheckError("Subset membership was accepted")
