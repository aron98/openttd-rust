#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
evidence="${1:?pass an isolated rail evidence directory}"
mkdir -p "$evidence"
evidence="$(cd "$evidence" && pwd)"
cd "$root"
source_dir="$evidence/native-source"
build_dir="$evidence/native-build"
commit=14ec60f248547d4d062a1160f0fc26d742319888
if [ ! -d "$source_dir/.git" ]; then
    git clone --no-hardlinks "${REFERENCE_SOURCE:-https://github.com/OpenTTD/OpenTTD.git}" "$source_dir" > "$evidence/clone.log" 2>&1
    git -C "$source_dir" checkout --detach "$commit" >> "$evidence/clone.log" 2>&1
    git -C "$source_dir" apply "$root/reference/rail_consist.patch"
fi
test "$(git -C "$source_dir" rev-parse HEAD)" = "$commit"
verification_index="$(mktemp "$evidence/source-index.XXXXXX")"
rm "$verification_index"
trap 'rm -f "$verification_index"' EXIT
GIT_INDEX_FILE="$verification_index" git -C "$source_dir" read-tree HEAD
GIT_INDEX_FILE="$verification_index" git -C "$source_dir" apply --cached "$root/reference/rail_consist.patch"
GIT_INDEX_FILE="$verification_index" git -C "$source_dir" diff --exit-code > "$evidence/source-diff-check.log"
printf '%s\n' 'PASS tracked source equals pinned HEAD plus rail_consist.patch' >> "$evidence/source-diff-check.log"
for header in rail_consist rail_consist_fixture; do
    cp "$root/reference/$header.hpp" "$source_dir/src/saveload/reference_$header.hpp"
done
git -C "$source_dir" status --porcelain=v1 > "$evidence/native-git-status.txt"
git -C "$source_dir" ls-files --stage > "$evidence/native-git-index.txt"
git -C "$source_dir" diff --binary > "$evidence/native-source.patch"
git -C "$source_dir" rev-parse HEAD > "$evidence/native-pin.txt"
mkdir -p "$evidence/recipe-inputs"
cp "$root/reference/rail_consist.patch" "$root/reference/rail_consist.hpp" \
    "$root/reference/rail_consist_fixture.hpp" "$root/scripts/reference.cfg" \
    "$root/scripts/run-reference.cmake" "$root/scripts/check-rail-consist-reference.cmake" \
    "$0" "$evidence/recipe-inputs/"
(
    set -x
    cmake -S "$source_dir" -B "$build_dir" -DOPTION_DEDICATED=ON \
        -DOPTION_USE_ASSERTS=ON -DCMAKE_BUILD_TYPE=Release -DCMAKE_EXPORT_COMPILE_COMMANDS=ON
    cmake --build "$build_dir" --parallel "${JOBS:-4}"
) > "$evidence/build-invocation.log" 2>&1
archive="${REFERENCE_OPENGFX_TAR:?pass the original OpenGFX 7.1 tar path}"
tar -xf "$archive" -C "$build_dir/baseset"
(
    cd "$source_dir"
    git ls-files -z | xargs -0 shasum -a 256
    shasum -a 256 src/saveload/reference_rail_consist*.hpp
) > "$evidence/native-source.sha256"
shasum -a 256 "$build_dir/openttd" "$archive" "$evidence"/recipe-inputs/* > "$evidence/selected-inputs.sha256"
"$build_dir/openttd" -h > "$evidence/native-help.txt" 2>&1
printf '%s\n' "PASS pinned original source plus rail-only hooks; actual CMake invocation/build and input hashes retained" > "$evidence/build-result.txt"
