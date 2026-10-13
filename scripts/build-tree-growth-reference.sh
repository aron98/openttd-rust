#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-only
# Materialized native30 source is an explicit input, never the mutable shared source.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
source_input="${1:?materialized native30 source directory required}"
build_root="${2:?new private output directory required}"
python3 - "$source_input" "$root/reference/tree_growth.json" <<'CHECK'
import hashlib, json, sys
from pathlib import Path
source = Path(sys.argv[1])
metadata = json.loads(Path(sys.argv[2]).read_text())
for name, identity in metadata["native_source_inputs"].items():
    actual = hashlib.sha256((source / name).read_bytes()).hexdigest()
    if actual != identity["base_sha256"]:
        raise SystemExit(f"native30 source identity mismatch: {name}")
CHECK
test ! -e "$build_root"
mkdir -p "$build_root"
cp -R "$source_input" "$build_root/source"
(cd "$build_root/source" && patch -p1 < "$root/reference/tree_growth.patch")
cp "$root/reference/tree_growth_observer.hpp" "$build_root/source/src/reference_tree_growth_observer.hpp"
cmake -S "$build_root/source" -B "$build_root/build" -DCMAKE_BUILD_TYPE=Release -DOPTION_DEDICATED=ON -DOPTION_USE_ASSERTS=ON -DCMAKE_CXX_COMPILER_LAUNCHER=ccache
cmake --build "$build_root/build" --target openttd -j 4
