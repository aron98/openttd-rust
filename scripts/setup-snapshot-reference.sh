#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
commit="$(sed -n 's/^commit = "\([^"]*\)"/\1/p' upstream.toml)"
repository="$(sed -n 's/^repository = "\([^"]*\)"/\1/p' upstream.toml)"
source_dir="$root/.reference/snapshot-source"
build_dir="$root/.reference/snapshot-build"
if [ ! -d "$source_dir/.git" ]; then
    git clone --no-hardlinks "${REFERENCE_SOURCE:-$repository}" "$source_dir"
    git -C "$source_dir" checkout --detach "$commit"
fi
test "$(git -C "$source_dir" rev-parse HEAD)" = "$commit"
if git -C "$source_dir" apply --check "$root/reference/snapshot.patch" 2>/dev/null; then
    git -C "$source_dir" apply "$root/reference/snapshot.patch"
else
    git -C "$source_dir" apply --reverse --check "$root/reference/snapshot.patch"
fi
# Compare tracked source with precisely HEAD plus our patch, without changing its index.
verification_index="$(mktemp "$root/.reference/snapshot-index.XXXXXX")"
rm "$verification_index"
trap 'rm -f "$verification_index"' EXIT
GIT_INDEX_FILE="$verification_index" git -C "$source_dir" read-tree HEAD
GIT_INDEX_FILE="$verification_index" git -C "$source_dir" apply --cached "$root/reference/snapshot.patch"
GIT_INDEX_FILE="$verification_index" git -C "$source_dir" diff --exit-code
cp reference/snapshot.hpp "$source_dir/src/saveload/reference_snapshot.hpp"
cmake -S "$source_dir" -B "$build_dir" \
    -DOPTION_DEDICATED=ON -DOPTION_USE_ASSERTS=ON -DCMAKE_BUILD_TYPE=Release
cmake --build "$build_dir" --parallel "${JOBS:-4}"
archive="${REFERENCE_OPENGFX_ARCHIVE:-$root/.reference/opengfx-7.1-all.zip}"
if [ ! -f "$archive" ]; then
    curl --fail --location --max-time 120 https://cdn.openttd.org/opengfx-releases/7.1/opengfx-7.1-all.zip --output "$archive.part"
    mv "$archive.part" "$archive"
fi
actual="$(cmake -E sha256sum "$archive")"
test "${actual%% *}" = 928fcf34efd0719a3560cbab6821d71ce686b6315e8825360fba87a7a94d7846
unzip -o "$archive" -d "$root/.reference"
tar -xf "$root/.reference/opengfx-7.1.tar" -C "$build_dir/baseset"
"$build_dir/openttd" -h
