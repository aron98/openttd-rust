#!/usr/bin/env bash
# Add passive annual/minimum-age observations to a prepared pinned source.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
PYTHONPATH="$root" python3 -m scripts.vehicle_year_sources "$root"
source_dir="${1:?prepared original source directory required}"
bash "$root/scripts/setup-cargo-observer.sh" "$source_dir"
cp -p "$root/reference/replay_hooks.hpp" "$source_dir/src/reference_replay_hooks.hpp"
for header in replay_cost replay_commands; do
    cp -p "$root/reference/$header.hpp" "$source_dir/src/saveload/reference_$header.hpp"
done
cp -p "$root/reference/vehicle_year_trace.hpp" "$source_dir/src/saveload/reference_vehicle_year_trace.hpp"
if ! git -C "$source_dir" apply --reverse --check "$root/reference/vehicle_year.patch" 2>/dev/null; then
    git -C "$source_dir" apply --check "$root/reference/vehicle_year.patch"
    git -C "$source_dir" apply "$root/reference/vehicle_year.patch"
fi
