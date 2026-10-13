from __future__ import annotations

from pathlib import Path

from scripts.context_ci_support import invocation_fields
from scripts.gameplay_foundations import digest
from scripts.grf_control_evidence import sequence, text
from scripts.language_ci_compare import compare, mapping
from scripts.world_check_support import Json, WorldCheckError, at, read_json


def membership(actual: list[str], expected: list[str]) -> None:
    if not expected or len(set(expected)) != len(expected) or actual != expected:
        raise WorldCheckError(
            "Tree case membership is zero, subset, duplicate or reordered"
        )


def native_case(root: Path, output: Path, oracle: Path, entry: Json) -> Json:
    identity = text(at(entry, ("id",)))
    group, name = identity.split("/", 1)
    directory = output / group / "runs" / name
    native = read_json(directory / "results.json")
    compare(read_json(directory / "actions.json"), at(entry, ("protocol",)))
    if (directory / "process.txt").read_text() != "exit=0\n":
        raise WorldCheckError("Tree original process failed")
    fields = invocation_fields(root, directory / "invocation.txt")
    compare(str(Path(fields["ORACLE"]).resolve()), str(oracle.resolve()))
    compare(fields["ORACLE_SHA256"], digest(oracle))
    compare(digest(directory / "openttd.cfg"), digest(root / "scripts/reference.cfg"))
    expected_sources = sorted(
        f"SOURCE_SHA256={digest(path)} {path.relative_to(root)}"
        for extension in ("*.hpp", "*.patch")
        for path in (root / "reference").glob(extension)
    )
    recorded_sources = sorted(
        line
        for line in (directory / "invocation.txt").read_text().splitlines()
        if line.startswith("SOURCE_SHA256=")
    )
    compare([*recorded_sources], [*expected_sources])
    for key in ("INPUT", "REPLAY"):
        compare(fields[key + "_SHA256"], digest(Path(fields[key])))
    actions = sequence(at(native, ("actions",)))
    expected_actions = sequence(at(entry, ("protocol", "actions")))
    compare(len(actions), len(expected_actions))
    for observed, requested in zip(actions, expected_actions, strict=True):
        compare(at(observed, ("op",)), at(requested, ("op",)))
        compare(at(observed, ("ordinal",)), at(requested, ("ordinal",)))
        for field in ("random", "interactive_random"):
            compare(at(observed, ("before", field)), at(observed, ("after", field)))
    labels = [
        text(at(row, ("label",))) for row in sequence(at(native, ("checkpoints",)))
    ]
    events = 0
    for action in actions:
        if at(action, ("op",)) != "command":
            continue
        metadata = mapping(at(action, ("native_metadata",)))
        if at(entry, ("observe",)) is True:
            compare(at(metadata, ("tree_rating", "schema_version")), 1)
            observed = sequence(at(metadata, ("tree_rating", "events")))
            compare(
                [at(row, ("ordinal",)) for row in observed], list(range(len(observed)))
            )
            events += len(observed)
        elif "tree_rating" in metadata:
            raise WorldCheckError("Observer-off native command emitted a trace")
    for label in labels:
        runtime = read_json(directory / (label + ".runtime.json"))
        compare(at(runtime, ("before_save",)), at(runtime, ("after_save",)))
    observed: Json = {
        "actions": len(actions),
        "commands": sum(at(row, ("op",)) == "command" for row in actions),
        "events": events,
        "checkpoints": [*labels],
    }
    compare(observed, at(entry, ("historical_membership",)))
    actual_files = sorted(p.name for p in directory.iterdir() if p.is_file())
    expected_files = sorted(
        [
            label + "." + extension
            for label in labels
            for extension in (
                "sav",
                "world.json",
                "derived.json",
                "schema.json",
                "runtime.json",
            )
        ]
        + [
            "actions.json",
            "results.json",
            "invocation.txt",
            "checkpoint-hashes.txt",
            "openttd.cfg",
            "process.txt",
            "stdout.log",
            "stderr.log",
        ]
    )
    compare([*actual_files], [*expected_files])
    return {
        "id": identity,
        "observed": observed,
        "files": [*actual_files],
        "hashes": {name: digest(directory / name) for name in actual_files},
    }


def normal_rust(output: Path, domains: Json) -> Json:
    expected_files: list[str] = []
    summaries: list[str] = []
    total_commands = total_events = 0
    observed_root = output / "tests/normal_corpus-0/observations"
    for domain in sequence(domains):
        if at(domain, ("domain",)) != "terrain_parity":
            continue
        name = text(at(domain, ("id",)))
        native = read_json(output / "corpus/runs" / name / "results.json")
        commands = events = 0
        for action in sequence(at(native, ("actions",))):
            if at(action, ("op",)) != "command":
                continue
            file = f"normal-{name}-{at(action, ('ordinal',))}.json"
            expected_files.append(file)
            actual = read_json(observed_root / file)
            compare(at(actual, ("receipt",)), at(action, ("receipt",)))
            compare(at(actual, ("public_receipt",)), at(action, ("receipt",)))
            trace = at(action, ("native_metadata", "tree_rating", "events"))
            compare(at(actual, ("events",)), trace)
            commands += 1
            events += len(sequence(trace))
        summary = f"normal-{name}.json"
        expected_files.append(summary)
        compare(
            read_json(observed_root / summary),
            {
                "case": "normal-" + name,
                "commands": commands,
                "events": events,
                "receipt_saved_derived_match": True,
                "trace_compared": True,
            },
        )
        summaries.append(name)
        total_commands += commands
        total_events += events
    compare(
        [*sorted(p.name for p in observed_root.iterdir())], [*sorted(expected_files)]
    )
    if len(summaries) != 45 or total_commands != 78 or total_events != 2713:
        raise WorldCheckError("Incomplete ordinary Rust tree corpus")
    return {
        "cases": [*summaries],
        "commands": total_commands,
        "events": total_events,
        "files": [*sorted(expected_files)],
    }


def split_resume(output: Path) -> None:
    runs = output / "corpus/runs"
    for extension in ("sav", "world.json", "derived.json", "schema.json"):
        compare(
            digest(runs / "sequence" / ("final." + extension)),
            digest(runs / "resume" / ("final." + extension)),
        )
    continuous = [
        row
        for row in sequence(at(read_json(runs / "sequence/results.json"), ("actions",)))
        if at(row, ("ordinal",)) in (3, 4)
    ]
    resumed = sequence(at(read_json(runs / "resume/results.json"), ("actions",)))
    for actions in (continuous, resumed):
        for row in actions:
            for phase in ("before", "after"):
                _ = mapping(at(row, (phase,))).pop("interactive_random")
    compare(continuous, resumed)
    compare(
        read_json(runs / "sequence/final.world.json"),
        read_json(runs / "reload-sequence/initial.world.json"),
    )


def observer_and_shore_pairs(output: Path) -> None:
    runs = output / "corpus/runs"
    for first, second in [
        ("coastal-clear", "coastal-clear-off"),
        ("coastal-clear", "coastal-clear-repeat"),
        ("coastal-clear", "coastal-dry-clear"),
        ("coastal-terraform-lower", "coastal-dry-terraform"),
    ]:
        for extension in ("sav", "world.json", "derived.json", "schema.json"):
            compare(
                digest(runs / first / ("final." + extension)),
                digest(runs / second / ("final." + extension)),
            )
        before = read_json(runs / first / "results.json")
        after = read_json(runs / second / "results.json")
        compare(
            [at(row, ("receipt",)) for row in sequence(at(before, ("actions",)))],
            [at(row, ("receipt",)) for row in sequence(at(after, ("actions",)))],
        )


def reload_stability(output: Path) -> None:
    runs = output / "corpus/runs"
    for first, reload in [
        ("canonical-climate-1", "reload-climate-1"),
        ("canonical-climate-desert", "reload-climate-desert"),
        ("sequence", "reload-sequence"),
    ]:
        for extension in ("world.json", "derived.json", "schema.json"):
            expected = read_json(runs / first / ("final." + extension))
            for phase in ("initial", "final"):
                compare(read_json(runs / reload / (phase + "." + extension)), expected)
