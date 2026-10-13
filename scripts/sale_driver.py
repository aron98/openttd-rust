# SPDX-License-Identifier: GPL-2.0-only
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
# Run: bash scripts/check-road-sale.sh
"""Bounded native sale matrix with immutable execution evidence."""

import os
import tempfile
from pathlib import Path

from scripts.gameplay_foundations import FoundationRun, digest, require_test
from scripts.sale_evidence import package
from scripts.sale_provenance import prepare
from scripts.world_check_support import (
    ROOT,
    Json,
    WorldCheckError,
    read_json,
    write_json,
)


def source_hashes(root: Path) -> Json:
    paths = [
        root / "Cargo.toml",
        root / "Cargo.lock",
        root / "upstream.toml",
        root / "fixtures/replay/clear-v362.sav",
    ]
    for folder in ("crates", "scripts", "reference"):
        paths.extend(
            path
            for path in (root / folder).rglob("*")
            if path.is_file()
            and path.suffix
            in (".rs", ".py", ".toml", ".json", ".sh", ".cmake", ".hpp", ".cfg")
        )
    return {str(path.relative_to(root)): digest(path) for path in sorted(paths)}


def main() -> None:
    parent = ROOT / ".artifacts"
    parent.mkdir(exist_ok=True)
    output = Path(tempfile.mkdtemp(prefix="road-sale-", dir=parent))
    print(f"Artifacts: {output}", flush=True)
    oracle = Path(
        os.environ.get(
            "OTTD_SALE_ORACLE", str(ROOT / ".reference/snapshot-build/openttd")
        )
    ).resolve(strict=True)
    write_json(
        output / "allocator-environment.json",
        {
            name: os.environ.get(name)
            for name in ("MallocScribble", "MallocPreScribble", "MallocNanoZone")
        },
    )
    runner = FoundationRun(ROOT, output, oracle)
    _ = runner.run("build-cli", ["cargo", "build", "--locked", "-p", "ottd-cli"])
    cli = (
        Path(os.environ.get("CARGO_TARGET_DIR", str(ROOT / "target"))) / "debug/ottd"
    ).resolve(strict=True)
    prepare(output, oracle)
    before = source_hashes(ROOT)
    binaries: Json = {"native": digest(oracle), "cli": digest(cli)}
    write_json(output / "before.json", {"source": before, "binaries": binaries})
    zero = runner.run(
        "zero-test",
        [
            "cargo",
            "test",
            "--locked",
            "-p",
            "ottd-sim",
            "--lib",
            "sale_ci_deliberately_missing_selector",
            "--",
            "--ignored",
            "--exact",
        ],
    )
    try:
        require_test(zero.stdout, "sale_ci_deliberately_missing_selector")
    except WorldCheckError:
        write_json(
            output / "zero-test-rejected.json", {"rejected": True, "executed": 0}
        )
    else:
        raise WorldCheckError("Zero-test execution was accepted")
    _ = runner.run(
        "matrix",
        [
            "python3",
            "-m",
            "scripts.sale_matrix",
            "--oracle",
            str(oracle),
            "--ottd",
            str(cli),
            "--artifacts",
            str(output / "results"),
        ],
    )
    after: Json = {
        "source": source_hashes(ROOT),
        "binaries": {"native": digest(oracle), "cli": digest(cli)},
    }
    write_json(output / "after.json", after)
    if read_json(output / "before.json") != after:
        raise WorldCheckError("Sale sources or binaries changed during execution")
    package(output)
    _ = (output / "summary.txt").write_text("PASS owned-runtime fresh road sales\n")
    print("PASS owned-runtime fresh road sales", flush=True)


if __name__ == "__main__":
    main()
