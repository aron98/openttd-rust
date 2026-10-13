#!/usr/bin/env bash
set -euo pipefail
source_dir="${1:?usage: setup-async-trace-reference.sh PREPARED_SOURCE}"
root="$(cd "$(dirname "$0")/.." && pwd)"
PYTHONPATH="$root" python3 -m scripts.async_trace_sources "$root"
source_dir="$(cd "$source_dir" && pwd)"
pin=14ec60f248547d4d062a1160f0fc26d742319888
[[ "$(git -C "$source_dir" rev-parse HEAD)" == "$pin" ]]
# Establish the unchanged checkpoint observer foundation before the additive patch.
bash "$root/scripts/setup-cargo-observer.sh" "$source_dir"
if ! git -C "$source_dir" apply --reverse --check "$root/reference/async_trace.patch" 2>/dev/null; then
    git -C "$source_dir" apply --check "$root/reference/async_trace.patch"
    git -C "$source_dir" apply "$root/reference/async_trace.patch"
fi
cp "$root/reference/async_trace.hpp" "$source_dir/src/reference_async_trace.hpp"
for part in state input jobs; do
    cp "$root/reference/async_trace_${part}.hpp" "$source_dir/src/saveload/reference_async_trace_${part}.hpp"
done
