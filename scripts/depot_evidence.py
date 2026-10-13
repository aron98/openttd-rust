from pathlib import Path

from scripts.gameplay_foundations import require_files
from scripts.world_check_support import WorldCheckError, decode_json


def require_depot_evidence(root: Path) -> None:
    names = [
        "comparison.txt",
        "rust.runtime.json",
        "rust.vectors.json",
        *[f"rejected-{name}.json" for name in ["pool", "road", "tram", "high-slot"]],
        *[
            f"{phase}/{name}"
            for phase in ["prepare", "canonical", "loaded", "reload"]
            for name in [
                "depot-runtime.json",
                "invocation.txt",
                "openttd.cfg",
                "stderr.log",
                "save/autosave/exit.sav",
            ]
        ],
    ]
    require_files(root, names)
    if (root / "comparison.txt").read_text() != (
        "PASS canonical depot/road restore; 428 counter-only vectors; "
        "4 rejected controls; native reload\n"
    ):
        raise WorldCheckError("Depot comparison coverage changed")
    native = decode_json((root / "loaded/depot-runtime.json").read_text())
    rust = decode_json((root / "rust.runtime.json").read_text())
    vectors = decode_json((root / "rust.vectors.json").read_text())
    match native:
        case {"runtime": runtime, "counter_vectors": list() as cases}:
            if runtime != rust or len(cases) != 428:
                raise WorldCheckError("Depot native runtime or vector count differs")
        case _:
            raise WorldCheckError("Missing depot native runtime/vectors")
    match rust:
        case {"pool": pool, "road": dict() as companies}:
            if pool != {
                "first_free": 0,
                "first_unused": 131,
                "items": 2,
                "occupied": [0, 130],
                "slots": 192,
            } or set(companies) != {"0", "1"}:
                raise WorldCheckError("Depot sparse allocator/company coverage changed")
            for counters in companies.values():
                match counters:
                    case list() if len(counters) == 63:
                        continue
                    case _:
                        raise WorldCheckError(
                            "Depot requires every native counter slot"
                        )
        case _:
            raise WorldCheckError("Missing restored depot pool/company counters")
    match vectors:
        case list() if len(vectors) == 428:
            for actual, case in zip(vectors, cases, strict=True):
                match case:
                    case {"runtime": expected} if actual == expected:
                        continue
                    case _:
                        raise WorldCheckError("Depot counter vector differs")
        case _:
            raise WorldCheckError("Missing restored depot vectors")
