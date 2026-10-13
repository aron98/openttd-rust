from __future__ import annotations

from pathlib import Path

from scripts.backup_sale_run import CONFIG
from scripts.context_ci_support import exact
from scripts.gameplay_foundations import digest
from scripts.owned_restore_run import MALLOC, RestoreRun
from scripts.world_check_support import Json, WorldCheckError, read_json


def native_receipt(job: RestoreRun, case: Path, source: Path) -> None:
    expected: Json = {
        "source": str(source),
        "source_sha256": digest(source),
        "descriptor_sha256": digest(case / "actions.json"),
        "oracle_sha256": digest(job.oracle),
    }
    if not exact(read_json(case / "inputs.json"), expected):
        raise WorldCheckError("Restore input identity differs")
    native_directory = case / "original"
    paths = (
        job.oracle,
        source,
        case / "actions.json",
        native_directory / "openttd.cfg",
    )
    expected = {str(path): digest(path) for path in paths}
    for phase in ("before", "after"):
        if not exact(read_json(native_directory / f"bindings-{phase}.json"), expected):
            raise WorldCheckError("Restore native source or executable changed")
    native_argv: list[Json] = [
        str(job.oracle),
        "-x",
        "-c",
        str(native_directory / "openttd.cfg"),
        "-snull",
        "-mnull",
        "-vnull:ticks=100000000",
        "-g",
        str(source),
    ]
    if not exact(read_json(native_directory / "argv.json"), native_argv) or not exact(
        read_json(native_directory / "process.json"),
        {"returncode": 0, "expected": 0, "timeout_seconds": 180},
    ):
        raise WorldCheckError("Restore native invocation differs")
    variables: dict[str, Json] = {
        job.observer_mode: "1",
        "OTTD_ORDER_STATE_PATH": str(native_directory / "loaded.json"),
        "OTTD_WORLD_PATH": str(native_directory / "saved-world.json"),
        "OTTD_WORLD_SCHEMA_PATH": str(native_directory / "saved-schema.json"),
        "OTTD_ORDER_FIXTURE_PATH": str(case / "actions.json"),
        "OTTD_ORDER_FIXTURE_DIR": str(native_directory / "native"),
        **dict.fromkeys(MALLOC),
    }
    if (
        not exact(read_json(native_directory / "environment.json"), variables)
        or (native_directory / "openttd.cfg").read_text() != CONFIG
    ):
        raise WorldCheckError("Restore native environment/configuration differs")
