from __future__ import annotations

from pathlib import Path

from scripts.context_ci_support import exact
from scripts.gameplay_foundations import digest
from scripts.order_state_evidence import command
from scripts.owned_restore_run import RestoreRun
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    decode_json,
    read_json,
    run,
    write_json,
)


def export(job: RestoreRun, source: Path, directory: Path) -> str:
    cli = job.executable("cli")
    before: Json = {"input_sha256": digest(source), "cli_sha256": digest(Path(cli))}
    result = run([cli, "world", str(source), "--view", "saved"], directory)
    after: Json = {"input_sha256": digest(source), "cli_sha256": digest(Path(cli))}
    write_json(directory / "bindings.json", {"before": before, "after": after})
    if not exact(before, after):
        raise WorldCheckError("Restore decode input or executable changed")
    return result.stdout


def verify_export(job: RestoreRun, source: Path, directory: Path) -> Json:
    cli = job.executable("cli")
    command(directory, [cli, "world", str(source), "--view", "saved"])
    expected: Json = {"input_sha256": digest(source), "cli_sha256": digest(Path(cli))}
    if not exact(
        read_json(directory / "bindings.json"), {"before": expected, "after": expected}
    ):
        raise WorldCheckError("Restore saved decode identity differs")
    return decode_json((directory / "stdout.log").read_text())
