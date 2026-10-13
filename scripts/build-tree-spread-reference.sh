#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-2.0-only
# Explicit parent: frozen native tree-growth/mixed-observer materialized source.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
source_input="${1:?frozen mixed tree-growth source directory required}"
build_root="${2:?new private output directory required}"
mode="${3:-normal}"
case "$mode" in normal|assertions) ;; *) exit 2 ;; esac
python3 - "$source_input" "$root/reference/tree_spread.json" <<'CHECK'
import hashlib, json, sys
from pathlib import Path
source = Path(sys.argv[1])
metadata = json.loads(Path(sys.argv[2]).read_text())
for name, identity in metadata["native_source_inputs"].items():
    if hashlib.sha256((source / name).read_bytes()).hexdigest() != identity["base_sha256"]:
        raise SystemExit(f"frozen native source identity mismatch: {name}")
CHECK
test ! -e "$build_root"
mkdir -p "$build_root"
cp -R "$source_input" "$build_root/source"
(cd "$build_root/source" && patch -p1 < "$root/reference/tree_spread.patch")
if [ "$mode" = assertions ]; then
    (cd "$build_root/source" && patch -p1 < "$root/reference/tree_spread_assert.patch")
fi
cmake -S "$build_root/source" -B "$build_root/build" -DCMAKE_BUILD_TYPE=Release -DOPTION_DEDICATED=ON -DOPTION_USE_ASSERTS=ON -DCMAKE_CXX_COMPILER_LAUNCHER=ccache
cmake --build "$build_root/build" --target openttd -j 4
