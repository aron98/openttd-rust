from __future__ import annotations

from pathlib import Path

from scripts.depot_removal_pair import projection
from scripts.gameplay_foundations import digest, require_test
from scripts.grf_control_evidence import sequence, text
from scripts.ordered_sale_run import SELECTOR, OrderedSaleRun, member
from scripts.world_check_support import (
    WorldCheckError,
    at,
    read_json,
    run,
    write_json,
)


def pair(job: OrderedSaleRun, name: str) -> None:
    case = member(job.output / "results", name)
    inputs = read_json(case / "inputs.json")
    source = Path(text(at(inputs, ("source",))))
    descriptor = read_json(case / "actions.json")
    cli = job.executable("cli")
    result = run(
        [
            "env",
            f"ORDERED_SALE_INPUT={source}",
            f"ORDERED_SALE_ACTIONS={case}/actions.json",
            f"ORDERED_SALE_OUTPUT={case}/rust",
            job.executable("runner"),
            "--exact",
            SELECTOR,
            "--ignored",
            "--nocapture",
        ],
        case / "rust-command",
    )
    require_test(result.stdout, SELECTOR)
    native = read_json(case / "original/native/results.json")
    projected = projection(native, descriptor)
    write_json(case / "native-state.json", projected)
    _ = run(
        [
            cli,
            "compare",
            str(case / "native-state.json"),
            str(case / "rust/results.json"),
        ],
        case / "live-compare",
    )
    saves = [
        text(at(action, ("label",)))
        for action in sequence(at(descriptor, ("actions",)))
        if at(action, ("op",)) == "save"
    ]
    if not saves or len(saves) != len(set(saves)):
        raise WorldCheckError("Depot saved checkpoint membership differs")
    for directory in (case / "original/native", case / "rust"):
        if {path.stem for path in directory.glob("*.sav")} != set(saves):
            raise WorldCheckError("Depot save omitted or added")
    for label in saves:
        for kind, directory in (
            ("native", case / "original/native"),
            ("rust", case / "rust"),
        ):
            exported = run(
                [cli, "world", str(directory / f"{label}.sav"), "--view", "saved"],
                case / f"{label}-{kind}-export",
            )
            _ = (case / f"{label}-{kind}.json").write_text(exported.stdout)
        _ = run(
            [
                cli,
                "compare",
                str(case / f"{label}-native.json"),
                str(case / f"{label}-rust.json"),
            ],
            case / f"{label}-compare",
        )
    _ = run(
        [
            cli,
            "compare",
            str(case / "original/saved-world.json"),
            str(case / f"{saves[-1]}-rust.json"),
        ],
        case / "serializer-compare",
    )
    if digest(source) != at(inputs, ("source_sha256",)) or digest(
        case / "actions.json"
    ) != at(inputs, ("descriptor_sha256",)):
        raise WorldCheckError("Ordered sale source input changed")
    print(f"PASS original ordered sale {name}", flush=True)
