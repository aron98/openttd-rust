from __future__ import annotations

import shutil
from pathlib import Path

from scripts.context_ci_support import sources
from scripts.owned_restore_provenance import verify_all as verify_build
from scripts.owned_restore_run import RestoreRun
from scripts.owned_restore_sources import verify_compiled
from scripts.shared_restore_layout import source_names
from scripts.world_check_support import Json, WorldCheckError, at


def snapshot(root: Path, output: Path, layout: Json) -> None:
    verify_closure(root, layout)
    sources(root, layout)
    verify_compiled(root, at(layout, ("sources",)))
    match at(layout, ("sources",)):
        case dict() as entries:
            for name in entries:
                destination = output / "source" / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                _ = shutil.copy2(root / name, destination)
        case _:
            raise WorldCheckError("Missing Restore source closure")
    _ = shutil.copy2(
        root / "scripts/shared-restore-layout.json", output / "layout.json"
    )


def verify_closure(root: Path, layout: Json) -> None:
    match at(layout, ("sources",)):
        case dict() as entries if set(entries) == set(source_names(root)):
            return
        case _:
            raise WorldCheckError("Shared source closure membership differs")


def verify_all(job: RestoreRun, layout: Json) -> None:
    verify_closure(job.root, layout)
    verify_build(job, layout)
