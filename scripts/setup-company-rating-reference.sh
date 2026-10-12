#!/usr/bin/env bash
# Add passive company-month observations to the prepared annual observer source.
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
PYTHONPATH="$root" python3 -m scripts.company_rating_sources "$root"
source_dir="${1:?prepared original source directory required}"
bash "$root/scripts/setup-vehicle-year-reference.sh" "$source_dir"
cp -p "$root/reference/company_rating_trace.hpp" "$source_dir/src/saveload/reference_company_rating_trace.hpp"
if ! git -C "$source_dir" apply --reverse --check "$root/reference/company_rating.patch" 2>/dev/null; then
    git -C "$source_dir" apply --check "$root/reference/company_rating.patch"
    git -C "$source_dir" apply "$root/reference/company_rating.patch"
fi
