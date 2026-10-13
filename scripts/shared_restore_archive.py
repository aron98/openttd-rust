from __future__ import annotations

import argparse
import shutil
from pathlib import Path

from scripts.depot_build_archive import package_raw, verify_archive
from scripts.gameplay_foundations import digest
from scripts.owned_restore_run import RestoreRun
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    read_json,
    run,
    write_json,
)


def archive_controls(job: RestoreRun) -> None:
    root = job.output / "controls/archive"
    baseline = root / "baseline"
    baseline.mkdir(parents=True)
    source = job.output / "results/cases/two-member"
    files = {
        "actions.json": source / "actions.json",
        "after.sav": source / "rust/after.sav",
        "rust.stdout.log": source / "rust-command/stdout.log",
    }
    before: dict[str, Json] = {str(path): digest(path) for path in files.values()}
    for name, path in files.items():
        _ = shutil.copy2(path, baseline / name)
    package_raw(baseline)
    _ = run(
        ["python3", "-m", "scripts.shared_restore_archive", str(baseline)],
        root / "commands/baseline",
    )
    for name, diagnostic in (
        ("raw", "archived raw evidence changed"),
        ("archive", "archive digest changed"),
        ("index", "Invalid depot archive index"),
    ):
        directory = root / name
        _ = shutil.copytree(baseline, directory)
        if name == "index":
            index = read_json(directory / "evidence-index.json")
            match index:
                case dict() as fields:
                    fields["count"] = 0
                case _:
                    raise WorldCheckError("Archive control index missing")
            write_json(directory / "evidence-index.json", index)
        else:
            path = directory / ("after.sav" if name == "raw" else "evidence.tar.gz")
            data = path.read_bytes()
            _ = path.write_bytes(data[:-1] + bytes([data[-1] ^ 1]))
        rejected = run(
            ["python3", "-m", "scripts.shared_restore_archive", str(directory)],
            root / "commands" / name,
            1,
        )
        if diagnostic not in rejected.stderr:
            raise WorldCheckError("Archive corruption rejected for wrong reason")
    if before != {str(path): digest(path) for path in files.values()}:
        raise WorldCheckError("Archive probes changed source evidence")
    write_json(root / "bindings.json", {"before": before, "after": before})


class Arguments(argparse.Namespace):
    directory: Path = Path()


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Validate actual raw/index/archive closure"
    )
    _ = parser.add_argument("directory", type=Path)
    args = Arguments()
    _ = parser.parse_args(namespace=args)
    verify_archive(args.directory)


if __name__ == "__main__":
    main()
