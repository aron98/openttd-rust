#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-only
# Input is the frozen, materialized tree-growth source epoch, not native30.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
source_input="${1:?materialized tree-growth source directory required}"
build_root="${2:?new private output directory required}"
python3 - "$source_input" "$root/reference" <<'CHECK'
import hashlib, json, sys
from pathlib import Path
source, reference = map(Path, sys.argv[1:])
metadata = json.loads((reference / "tree_growth.json").read_text())
expected = {name: entry["patched_sha256"] for name, entry in metadata["native_source_inputs"].items()}
expected["src/reference_tree_growth_observer.hpp"] = metadata["observer"]["sha256"]
mixed = json.loads((reference / "tree_growth_mixed.json").read_text())
expected[mixed["input"]] = mixed["base_sha256"]
for name, identity in expected.items():
    if hashlib.sha256((source / name).read_bytes()).hexdigest() != identity:
        raise SystemExit(f"tree-growth source identity mismatch: {name}")
CHECK
test ! -e "$build_root"
mkdir -p "$build_root"
cp -R "$source_input" "$build_root/source"
(cd "$build_root/source" && patch -p1 < "$root/reference/tree_growth_mixed.patch")
cmake -S "$build_root/source" -B "$build_root/build" -DCMAKE_BUILD_TYPE=Release -DOPTION_DEDICATED=ON -DOPTION_USE_ASSERTS=ON -DCMAKE_CXX_COMPILER_LAUNCHER=ccache
cmake --build "$build_root/build" --target openttd -j 4
