#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
commit="$(sed -n 's/^commit = "\([^"]*\)"/\1/p' upstream.toml)"
release="$(sed -n 's/^release = "\([^"]*\)"/\1/p' upstream.toml)"
repository="$(sed -n 's/^repository = "\([^"]*\)"/\1/p' upstream.toml)"
test -n "$commit" && test -n "$release" && test -n "$repository"

if [ ! -d .reference/OpenTTD/.git ]; then
    git clone --depth 1 --branch "$release" "$repository" .reference/OpenTTD
fi
test "$(git -C .reference/OpenTTD rev-parse HEAD)" = "$commit"
git -C .reference/OpenTTD diff --exit-code HEAD

cmake -S .reference/OpenTTD -B .reference/build \
    -DOPTION_DEDICATED=ON -DOPTION_USE_ASSERTS=ON -DCMAKE_BUILD_TYPE=Release \
    "-DCMAKE_C_COMPILER_LAUNCHER=${REFERENCE_COMPILER_LAUNCHER:-}" \
    "-DCMAKE_CXX_COMPILER_LAUNCHER=${REFERENCE_COMPILER_LAUNCHER:-}" \
    "-DCMAKE_DISABLE_PRECOMPILE_HEADERS=${REFERENCE_DISABLE_PCH:-OFF}"
cmake --build .reference/build --parallel "${JOBS:-4}"

archive=.reference/opengfx-7.1-all.zip
if [ ! -f "$archive" ]; then
    curl --fail --location --max-time 120 \
        https://cdn.openttd.org/opengfx-releases/7.1/opengfx-7.1-all.zip \
        --output "$archive.part"
    mv "$archive.part" "$archive"
fi
actual="$(cmake -E sha256sum "$archive")"
test "${actual%% *}" = 928fcf34efd0719a3560cbab6821d71ce686b6315e8825360fba87a7a94d7846
unzip -o "$archive" -d .reference
tar -xf .reference/opengfx-7.1.tar -C .reference/build/baseset
.reference/build/openttd -h
