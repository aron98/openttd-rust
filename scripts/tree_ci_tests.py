from __future__ import annotations

from pathlib import Path

from scripts.context_ci_support import process
from scripts.gameplay_foundations import log_name
from scripts.grf_control_evidence import sequence, text
from scripts.language_ci_compare import compare, mapping
from scripts.tree_ci_run import TreeRun
from scripts.world_check_support import Json, WorldCheckError, at, read_json, write_json


def execute(run: TreeRun, lib: Path, selectors: Json) -> None:
    for group, items in mapping(selectors).items():
        for ordinal, item in enumerate(sequence(items)):
            selector = text(item)
            directory = run.job.output / "tests" / f"{group}-{ordinal}"
            variables = {
                "TREE_NATIVE_CORPUS": str(run.job.output / "corpus"),
                "TREE_NATIVE_AUDIT": str(run.job.output / "prior"),
                "TREE_CORPUS_OUTPUT": str(directory / "observations"),
                "TREE_RUST_TRACE_DIR": str(directory / "legacy-observations"),
            }
            run.test(lib, selector, variables)
            process(run.job.output / "logs" / log_name(selector), lib, selector)


def validate(run: TreeRun, lib: Path, selectors: Json) -> Json:
    membership: dict[str, Json] = {}
    for group, items in mapping(selectors).items():
        for ordinal, item in enumerate(sequence(items)):
            selector = text(item)
            process(run.job.output / "logs" / log_name(selector), lib, selector)
            directory = run.job.output / "tests" / f"{group}-{ordinal}"
            membership[f"{group}-{ordinal}"] = list[Json](
                sorted(
                    str(p.relative_to(directory))
                    for p in directory.rglob("*")
                    if p.is_file()
                )
            )
    prior_cases = [
        ("clear-on", 0),
        ("threshold-inside", 0),
        ("threshold-boundary", 0),
        ("threshold-outside", 0),
        ("clamp", 0),
        ("below", 0),
        ("magic", 0),
        ("shore-clear", 0),
        ("rainforest", 0),
        ("tie", 0),
        ("clear-limit", 0),
        ("terraform-one", 1),
        ("terraform-four", 1),
        ("terraform-estimate", 1),
        ("terraform-limit", 1),
        ("terraform-money", 1),
        ("shore-terraform", 1),
        ("level-one", 2),
        ("level-money", 2),
    ]
    for name, group in prior_cases:
        native = read_json(run.job.output / "prior/runs" / name / "results.json")
        rust = read_json(
            run.job.output
            / f"tests/previous_milestones-{group}/legacy-observations/{name}.json"
        )
        compare(at(rust, ("receipt",)), at(native, ("actions", 0, "receipt")))
        compare(
            at(rust, ("events",)),
            at(native, ("actions", 0, "native_metadata", "tree_rating", "events")),
        )
    for index, distance in enumerate((0, 2)):
        for mode in ("estimate", "post"):
            row = read_json(
                run.job.output
                / f"tests/expected_red_admission-{index}/observations"
                / f"raw-{distance}-{mode}.json"
            )
            compare(
                at(row, ("free",)),
                {
                    "error": (
                        "unsupported command context: "
                        "noncanonical town authority distance"
                    )
                },
            )
            compare(at(row, ("runtime",)), at(row, ("free",)))
            compare(at(row, ("unchanged",)), rust=True)
    coupled = run.job.output / "tests/coupled_raw-0/observations"
    expected = sorted(
        f"coupled-{case}-{distance}-{mode}.json"
        for case in ("terraform-one", "level-one")
        for distance in (0, 2)
        for mode in ("estimate", "post")
    )
    compare([*sorted(p.name for p in coupled.iterdir())], [*expected])
    for name in expected:
        row = read_json(coupled / name)
        compare(at(row, ("testing",)), rust=False)
        compare(at(row, ("map_entries",)), 0)
        events = sequence(at(row, ("events",)))
        if (
            not events
            or at(events[-1], ("kind",)) != "scope_leave"
            or at(events[-1], ("depth",)) != 0
        ):
            raise WorldCheckError(
                "Coupled raw refusal did not complete actual scope cleanup"
            )
    for ordinal in (1, 2):
        row = read_json(
            run.job.output
            / f"tests/explicit_boundaries-1/observations/company-gate-{ordinal}.json"
        )
        compare(at(row, ("trace_parity",)), rust=False)
    write_json(run.job.output / "rust-membership.json", membership)
    return membership
