from __future__ import annotations

from pathlib import Path

from scripts.context_ci_support import exact
from scripts.depot_build_archive import bounded_paths
from scripts.depot_removal_pair import projection
from scripts.depot_removal_run import CONFIG, SELECTOR, RemovalRun, member
from scripts.gameplay_foundations import digest, require_test
from scripts.grf_control_evidence import sequence, text
from scripts.order_state_evidence import command
from scripts.world_check_support import Json, WorldCheckError, at, read_json


def membership(actual: Json, expected: Json) -> None:
    if not exact(actual, expected) or not sequence(expected):
        raise WorldCheckError("Exact depot removal case membership required")


def witnesses(native: Json, expected: Json) -> None:
    actual: list[Json] = [
        {"index": at(row, ("index",)), "receipt": at(row, ("result", "receipt"))}
        for row in sequence(at(native, ("actions",)))
        if at(row, ("input", "op")) == "command"
    ]
    membership(actual, expected)


def native_job(job: RemovalRun, entry: Json) -> None:
    results = job.output / "results"
    case = member(results, text(at(entry, ("directory",))))
    source = member(results, text(at(entry, ("source",))))
    native = case / "original"
    descriptor = at(entry, ("descriptor",))
    if (native / "openttd.cfg").read_text() != CONFIG:
        raise WorldCheckError("Pinned single-player config differs")
    if not exact(read_json(case / "actions.json"), descriptor):
        raise WorldCheckError("Pinned depot descriptor changed")
    before = read_json(native / "bindings-before.json")
    if not exact(before, read_json(native / "bindings-after.json")):
        raise WorldCheckError("Depot native before/after identity differs")
    expected: Json = {
        str(path): digest(path)
        for path in (job.oracle, source, case / "actions.json", native / "openttd.cfg")
    }
    if not exact(before, expected):
        raise WorldCheckError("Depot native source/executable/config identity differs")
    if not exact(
        read_json(native / "process.json"),
        {"returncode": 0, "expected": 0, "timeout_seconds": 90},
    ):
        raise WorldCheckError("Original depot command process failed")
    argv = [
        str(job.oracle),
        "-x",
        "-c",
        str(native / "openttd.cfg"),
        "-snull",
        "-mnull",
        "-d",
        "sl=2",
        "-vnull:ticks=100000000",
        "-g",
        str(source),
    ]
    if not exact(read_json(native / "argv.json"), list(argv)):
        raise WorldCheckError("Original depot process arguments changed")
    variables: Json = {
        "OTTD_DEPOT_REMOVAL_OBSERVE": "1",
        "OTTD_ORDER_STATE_PATH": str(native / "loaded.json"),
        "OTTD_WORLD_PATH": str(native / "saved-world.json"),
        "OTTD_WORLD_SCHEMA_PATH": str(native / "saved-schema.json"),
        "OTTD_ORDER_FIXTURE_PATH": str(case / "actions.json"),
        "OTTD_ORDER_FIXTURE_DIR": str(native / "native"),
    }
    if not exact(read_json(native / "environment.json"), variables):
        raise WorldCheckError("Depot observer environment changed")
    if not exact(
        read_json(case / "inputs.json"),
        {
            "source": str(source),
            "source_sha256": digest(source),
            "descriptor_sha256": digest(case / "actions.json"),
            "oracle_sha256": digest(job.oracle),
        },
    ):
        raise WorldCheckError("Depot input binding differs")


def edit_job(job: RemovalRun, entry: Json) -> None:
    results = job.output / "results"
    directory = member(results, text(at(entry, ("directory",))))
    source = member(results, text(at(entry, ("source",))))
    destination = member(results, text(at(entry, ("output",))))
    descriptor = destination.with_suffix(".edit.json")
    if not exact(read_json(descriptor), at(entry, ("edits",))):
        raise WorldCheckError("Pinned depot fixture edits differ")
    command(
        directory,
        [
            job.executable("cli"),
            "edit-world",
            str(source),
            str(descriptor),
            str(destination),
            "--compression",
            "none",
        ],
    )
    if not exact(
        read_json(directory / "bindings.json"),
        {
            "source": str(source),
            "source_before": digest(source),
            "source_after": digest(source),
            "output": str(destination),
            "output_sha256": digest(destination),
            "descriptor_sha256": digest(descriptor),
            "cli_sha256": digest(Path(job.executable("cli"))),
        },
    ):
        raise WorldCheckError("Depot generated input binding changed")


def pair_evidence(job: RemovalRun, name: str) -> tuple[int, int]:
    case = member(job.output / "results", name)
    inputs = read_json(case / "inputs.json")
    descriptor = read_json(case / "actions.json")
    source = text(at(inputs, ("source",)))
    runner, cli = job.executable("runner"), job.executable("cli")
    command(
        case / "rust-command",
        [
            "env",
            f"DEPOT_REMOVAL_INPUT={source}",
            f"DEPOT_REMOVAL_ACTIONS={case}/actions.json",
            f"DEPOT_REMOVAL_OUTPUT={case}/rust",
            runner,
            "--exact",
            SELECTOR,
            "--ignored",
            "--nocapture",
        ],
    )
    require_test((case / "rust-command/stdout.log").read_text(), SELECTOR)
    expected = projection(read_json(case / "original/native/results.json"), descriptor)
    actual = read_json(case / "rust/results.json")
    if not exact(expected, actual) or not exact(
        expected, read_json(case / "native-state.json")
    ):
        raise WorldCheckError("Complete depot command state differs")
    command(
        case / "live-compare",
        [
            cli,
            "compare",
            str(case / "native-state.json"),
            str(case / "rust/results.json"),
        ],
    )
    actions = sequence(at(descriptor, ("actions",)))
    labels = [text(at(a, ("label",))) for a in actions if at(a, ("op",)) == "save"]
    if not labels or len(set(labels)) != len(labels):
        raise WorldCheckError("Missing exact saved checkpoint membership")
    for folder in (case / "original/native", case / "rust"):
        if {p.stem for p in folder.glob("*.sav")} != set(labels):
            raise WorldCheckError("Depot checkpoint file membership differs")
    for label in labels:
        for kind, folder in (
            ("native", case / "original/native"),
            ("rust", case / "rust"),
        ):
            log = case / f"{label}-{kind}-export"
            command(
                log, [cli, "world", str(folder / f"{label}.sav"), "--view", "saved"]
            )
            if (log / "stdout.log").read_bytes() != (
                case / f"{label}-{kind}.json"
            ).read_bytes():
                raise WorldCheckError("Actual saved export changed")
        if not exact(
            read_json(case / f"{label}-native.json"),
            read_json(case / f"{label}-rust.json"),
        ):
            raise WorldCheckError("Full native/Rust saved state differs")
        command(
            case / f"{label}-compare",
            [
                cli,
                "compare",
                str(case / f"{label}-native.json"),
                str(case / f"{label}-rust.json"),
            ],
        )
    command(
        case / "serializer-compare",
        [
            cli,
            "compare",
            str(case / "original/saved-world.json"),
            str(case / f"{labels[-1]}-rust.json"),
        ],
    )
    if not exact(
        read_json(case / "original/saved-world.json"),
        read_json(case / f"{labels[-1]}-rust.json"),
    ):
        raise WorldCheckError("Original serializer differs")
    return len(actions), len(labels)


def validate(job: RemovalRun, layout: Json) -> Json:
    membership(at(job.fixtures, ("cases",)), at(layout, ("cases",)))
    cases = [text(v) for v in sequence(at(layout, ("cases",)))]
    membership(
        [at(entry, ("directory",)) for entry in sequence(at(job.fixtures, ("jobs",)))],
        at(layout, ("jobs",)),
    )
    for entry in sequence(at(job.fixtures, ("jobs",))):
        match at(entry, ("kind",)):
            case "native":
                native_job(job, entry)
            case "edit":
                edit_job(job, entry)
            case _:
                raise WorldCheckError("Unknown fixture admission operation")
    actions = saves = 0
    for name in cases:
        witnesses(
            read_json(
                member(job.output / "results", name) / "original/native/results.json"
            ),
            at(layout, ("command_witnesses", name)),
        )
        count, saved = pair_evidence(job, name)
        actions += count
        saves += saved
    coverage: Json = {
        "cases": len(cases),
        "actions": actions,
        "snapshots": len(cases) * 2 + actions * 2,
        "saves": saves,
    }
    if not exact(coverage, at(layout, ("coverage",))):
        raise WorldCheckError("Depot removal executed coverage differs")
    _ = bounded_paths(
        job.output / "results",
        {text(v) for v in sequence(at(layout, ("result_paths",)))},
    )
    return coverage
