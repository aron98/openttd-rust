from __future__ import annotations

import copy
import shutil
from collections.abc import Callable
from dataclasses import replace

from scripts.gameplay_foundations import digest, require_test
from scripts.owned_restore_evidence import save_membership, validate
from scripts.owned_restore_provenance import verify_all
from scripts.owned_restore_run import RestoreRun
from scripts.world_check_support import (
    Json,
    WorldCheckError,
    at,
    run,
    write_json,
)
from scripts.world_check_support import (
    replace as replace_json,
)


def rejected(check: Callable[[], None], diagnostic: str) -> str:
    try:
        check()
    except WorldCheckError as error:
        if diagnostic not in str(error):
            raise WorldCheckError(
                "Restore admission rejected for the wrong reason"
            ) from error
        return str(error)
    raise WorldCheckError("Restore admission unexpectedly accepted corruption")


def admission_controls(job: RestoreRun, layout: Json) -> None:
    output = job.output / "controls/admission"
    output.mkdir(parents=True)
    results: dict[str, Json] = {}
    result = run(
        [
            job.executable("runner"),
            "--exact",
            job.selector + "_absent",
            "--ignored",
            "--nocapture",
        ],
        output / "zero-test",
    )
    results["zero-test"] = rejected(
        lambda: require_test(result.stdout, job.selector),
        "Exact ignored test was not executed successfully",
    )
    _ = run(
        [
            "python3",
            str(job.root / "scripts/check-owned-restore.py"),
            "--case=owned-baseline",
        ],
        output / "subset-driver",
        1,
    )
    source_root = output / "wrong-source"
    _ = shutil.copytree(job.output / "source", source_root)
    embedded = source_root / "fixtures/world/populated-v362.sav"
    before = digest(embedded)
    data = embedded.read_bytes()
    _ = embedded.write_bytes(data[:-1] + bytes([data[-1] ^ 1]))
    write_json(
        output / "wrong-source-mutation.json",
        {
            "path": "fixtures/world/populated-v362.sav",
            "before_sha256": before,
            "after_sha256": digest(embedded),
            "byte_offset": len(data) - 1,
            "xor": 1,
        },
    )
    wrong_source = replace(job, root=source_root)
    results["wrong-source"] = rejected(
        lambda: verify_all(wrong_source, layout), "Pinned witness source changed"
    )
    changed_binary = copy.deepcopy(job.binaries)
    replace_json(changed_binary, ("runner",), copy.deepcopy(at(job.binaries, ("cli",))))
    wrong_binary = replace(job, binaries=changed_binary)
    write_json(output / "wrong-executable.json", changed_binary)
    results["wrong-executable"] = rejected(
        lambda: verify_all(wrong_binary, layout),
        "Restore executable differs from actual Cargo selection",
    )
    for name, remove in (("missing-case", True), ("extra-case", False)):
        fixtures = copy.deepcopy(job.fixtures)
        match at(fixtures, ("cases",)):
            case list() as entries:
                if remove:
                    _ = entries.pop()
                else:
                    entries.append(copy.deepcopy(entries[0]))
            case _:
                raise WorldCheckError("Missing Restore fixture cases")
        changed_job = replace(job, fixtures=fixtures)

        def check_roster(changed_job: RestoreRun = changed_job) -> None:
            _ = validate(changed_job)

        results[name] = rejected(check_roster, "29-case membership")
    for name in ("missing-save", "extra-save"):
        directory = output / name
        directory.mkdir()
        if name == "extra-save":
            source = job.output / "results/cases/owned-baseline/rust/after.sav"
            _ = shutil.copy2(source, directory / "after.sav")
            _ = shutil.copy2(source, directory / "extra.sav")
        results[name] = rejected(
            lambda directory=directory: save_membership(directory, ["after"]),
            "Restore save membership differs",
        )
    write_json(output / "results.json", results)
