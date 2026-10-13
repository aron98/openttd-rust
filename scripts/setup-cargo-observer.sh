#!/usr/bin/env bash
# Apply passive cargo/cache observations to a source prepared by setup-snapshot-reference.sh.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
PYTHONPATH="$root" python3 -m scripts.vehicle_year_sources "$root"
PYTHONPATH="$root" python3 -m scripts.company_rating_sources "$root"
source_dir="${1:?prepared original source directory required}"
pin="$(sed -n 's/^commit = "\([^"]*\)"/\1/p' "$root/upstream.toml")"
test "$(git -C "$source_dir" rev-parse HEAD)" = "$pin"
if ! git -C "$source_dir" apply --reverse --check "$root/reference/cargo_observer.patch" 2>/dev/null; then
    git -C "$source_dir" apply --check "$root/reference/cargo_observer.patch"
    git -C "$source_dir" apply "$root/reference/cargo_observer.patch"
fi
for header in cargo_runtime house_cache job_annotations industry_cache_snapshot group_profit_snapshot company_rating_snapshot; do
    cp "$root/reference/$header.hpp" "$source_dir/src/saveload/reference_$header.hpp"
done
cp "$root/reference/cargo_movement_snapshot.hpp" "$source_dir/src/saveload/reference_movement_snapshot.hpp"
cp "$root/reference/cargo_replay.hpp" "$source_dir/src/saveload/reference_replay.hpp"
