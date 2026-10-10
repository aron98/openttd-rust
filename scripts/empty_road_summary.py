from __future__ import annotations

from scripts.empty_road_evidence import membership, trace
from scripts.empty_road_run import EmptyRun
from scripts.grf_control_evidence import sequence, text
from scripts.language_ci_compare import compare, number
from scripts.world_check_support import Json, at, read_json


def summarize(run: EmptyRun) -> Json:
    pairs = sequence(at(run.fixtures, ("pairs",)))
    names = [text(at(row, ("id",))) for row in pairs]
    membership(names, names)
    output = run.job.output
    membership(
        sorted(path.name for path in (output / "native").iterdir()),
        sorted(["build", *names]),
    )
    membership(sorted(path.name for path in (output / "rust").iterdir()), sorted(names))
    counts = {
        "native_processes": len(names) + 1,
        "native_commands": 0,
        "native_ticks": 0,
        "native_checkpoints": 0,
        "rust_replays": len(names),
        "rust_ticks": 0,
        "rust_checkpoints": 0,
    }
    entries: list[tuple[str, str]] = [("build", "build")]
    entries.extend(
        (text(at(row, ("id",))), text(at(row, ("protocol",)))) for row in pairs
    )
    for name, protocol_name in entries:
        protocol = run.protocol(protocol_name)
        for kind in ("native",) if name == "build" else ("native", "rust"):
            result = read_json(output / kind / name / "results.json")
            trace(protocol, result, native=kind == "native")
            counts[kind + "_checkpoints"] += len(sequence(at(result, ("checkpoints",))))
            for action in sequence(at(protocol, ("actions",))):
                if at(action, ("op",)) == "tick":
                    counts[kind + "_ticks"] += number(at(action, ("count",)))
                if at(action, ("op",)) == "command":
                    counts["native_commands"] += 1
    actual: dict[str, Json] = dict(counts)
    compare(actual, at(run.fixtures, ("expected",)))
    return actual
