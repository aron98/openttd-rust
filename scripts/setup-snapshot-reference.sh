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
if ! git -C "$source_dir" apply --reverse --check "$root/reference/world.patch" 2>/dev/null; then
    for patch in snapshot gameplay world; do
        if git -C "$source_dir" apply --check "$root/reference/$patch.patch" 2>/dev/null; then
            git -C "$source_dir" apply "$root/reference/$patch.patch"
        else
            git -C "$source_dir" apply --reverse --check "$root/reference/$patch.patch"
        fi
    done
fi
if ! git -C "$source_dir" apply --reverse --check "$root/reference/order_fixture.patch" 2>/dev/null; then
    if git -C "$source_dir" apply --check "$root/reference/replay.patch" 2>/dev/null; then
        git -C "$source_dir" apply "$root/reference/replay.patch"
    else
        git -C "$source_dir" apply --reverse --check "$root/reference/replay.patch"
    fi
fi
# Compare tracked source with precisely HEAD plus our patch, without changing its index.
if ! git -C "$source_dir" apply --reverse --check "$root/reference/grf_currency.patch" 2>/dev/null &&
   ! git -C "$source_dir" apply --reverse --check "$root/reference/grf_strings.patch" 2>/dev/null &&
   ! git -C "$source_dir" apply --reverse --check "$root/reference/grf_language.patch" 2>/dev/null &&
   ! git -C "$source_dir" apply --reverse --check "$root/reference/grf_safety.patch" 2>/dev/null &&
   ! git -C "$source_dir" apply --reverse --check "$root/reference/grf_load_context.patch" 2>/dev/null; then
    if git -C "$source_dir" apply --check "$root/reference/grf_load_control.patch" 2>/dev/null; then
        git -C "$source_dir" apply "$root/reference/grf_load_control.patch"
    else
        git -C "$source_dir" apply --reverse --check "$root/reference/grf_load_control.patch"
    fi
    git -C "$source_dir" apply "$root/reference/grf_load_context.patch"
fi
if git -C "$source_dir" apply --check "$root/reference/road_slope.patch" 2>/dev/null; then
    git -C "$source_dir" apply "$root/reference/road_slope.patch"
else
    git -C "$source_dir" apply --reverse --check "$root/reference/road_slope.patch"
fi
if ! git -C "$source_dir" apply --reverse --check "$root/reference/grf_currency.patch" 2>/dev/null &&
   ! git -C "$source_dir" apply --reverse --check "$root/reference/grf_strings.patch" 2>/dev/null &&
   ! git -C "$source_dir" apply --reverse --check "$root/reference/grf_language.patch" 2>/dev/null; then
    if git -C "$source_dir" apply --check "$root/reference/grf_safety.patch" 2>/dev/null; then
        git -C "$source_dir" apply "$root/reference/grf_safety.patch"
    else
        git -C "$source_dir" apply --reverse --check "$root/reference/grf_safety.patch"
    fi
fi
for patch in order_state order_fixture order_network; do
    if git -C "$source_dir" apply --check "$root/reference/$patch.patch" 2>/dev/null; then
        git -C "$source_dir" apply "$root/reference/$patch.patch"
    else
        git -C "$source_dir" apply --reverse --check "$root/reference/$patch.patch"
    fi
done
if ! git -C "$source_dir" apply --reverse --check "$root/reference/grf_currency.patch" 2>/dev/null &&
   ! git -C "$source_dir" apply --reverse --check "$root/reference/grf_strings.patch" 2>/dev/null; then
    if git -C "$source_dir" apply --check "$root/reference/grf_language.patch" 2>/dev/null; then
        git -C "$source_dir" apply "$root/reference/grf_language.patch"
    else
        git -C "$source_dir" apply --reverse --check "$root/reference/grf_language.patch"
    fi
fi
if ! git -C "$source_dir" apply --reverse --check "$root/reference/grf_currency.patch" 2>/dev/null; then
    if git -C "$source_dir" apply --check "$root/reference/grf_strings.patch" 2>/dev/null; then
        git -C "$source_dir" apply "$root/reference/grf_strings.patch"
    else
        git -C "$source_dir" apply --reverse --check "$root/reference/grf_strings.patch"
    fi
fi
if git -C "$source_dir" apply --check "$root/reference/grf_currency.patch" 2>/dev/null; then
    git -C "$source_dir" apply "$root/reference/grf_currency.patch"
else
    git -C "$source_dir" apply --reverse --check "$root/reference/grf_currency.patch"
fi
if git -C "$source_dir" apply --check "$root/reference/tree_rating.patch" 2>/dev/null; then
    git -C "$source_dir" apply "$root/reference/tree_rating.patch"
else
    git -C "$source_dir" apply --reverse --check "$root/reference/tree_rating.patch"
fi
verification_index="$(mktemp "$root/.reference/snapshot-index.XXXXXX")"
rm "$verification_index"
trap 'rm -f "$verification_index"' EXIT
GIT_INDEX_FILE="$verification_index" git -C "$source_dir" read-tree HEAD
GIT_INDEX_FILE="$verification_index" git -C "$source_dir" apply --cached "$root/reference/snapshot.patch" "$root/reference/gameplay.patch" "$root/reference/world.patch" "$root/reference/replay.patch" "$root/reference/grf_load_control.patch" "$root/reference/grf_load_context.patch" "$root/reference/road_slope.patch" "$root/reference/grf_safety.patch" "$root/reference/order_state.patch" "$root/reference/order_fixture.patch" "$root/reference/order_network.patch" "$root/reference/grf_language.patch" "$root/reference/grf_strings.patch" "$root/reference/grf_currency.patch" "$root/reference/tree_rating.patch"
GIT_INDEX_FILE="$verification_index" git -C "$source_dir" diff --exit-code
copy_header() {
    if ! cmp -s "$1" "$2"; then cp "$1" "$2"; fi
}
copy_header reference/replay_hooks.hpp "$source_dir/src/reference_replay_hooks.hpp"
copy_header reference/grf_load_control.hpp "$source_dir/src/reference_grf_load_control.hpp"
copy_header reference/grf_load_context.hpp "$source_dir/src/reference_grf_load_context.hpp"
copy_header reference/road_slope.hpp "$source_dir/src/reference_road_slope.hpp"
copy_header reference/grf_safety.hpp "$source_dir/src/reference_grf_safety.hpp"
copy_header reference/grf_language.hpp "$source_dir/src/reference_grf_language.hpp"
copy_header reference/grf_language_catalog.hpp "$source_dir/src/reference_grf_language_catalog.hpp"
copy_header reference/grf_language_text.hpp "$source_dir/src/reference_grf_language_text.hpp"
copy_header reference/grf_strings.hpp "$source_dir/src/reference_grf_strings.hpp"
copy_header reference/grf_strings_api.hpp "$source_dir/src/reference_grf_strings_api.hpp"
copy_header reference/grf_currency.hpp "$source_dir/src/reference_grf_currency.hpp"
copy_header reference/grf_currency_api.hpp "$source_dir/src/reference_grf_currency_api.hpp"
copy_header reference/grf_currency_properties_api.hpp "$source_dir/src/reference_grf_currency_properties_api.hpp"
copy_header reference/tree_rating.hpp "$source_dir/src/reference_tree_rating.hpp"
copy_header reference/tree_rating_replay.hpp "$source_dir/src/saveload/reference_tree_rating_replay.hpp"
copy_header reference/tree_fixture.hpp "$source_dir/src/saveload/reference_tree_fixture.hpp"
copy_header reference/order_state.hpp "$source_dir/src/saveload/reference_order_state.hpp"
copy_header reference/order_fixture.hpp "$source_dir/src/saveload/reference_order_fixture.hpp"
copy_header reference/order_network.hpp "$source_dir/src/saveload/reference_order_network.hpp"
for header in replay replay_cost replay_commands replay_fixture; do
    copy_header "reference/$header.hpp" "$source_dir/src/saveload/reference_$header.hpp"
done
copy_header reference/snapshot.hpp "$source_dir/src/saveload/reference_snapshot.hpp"
copy_header reference/world.hpp "$source_dir/src/saveload/reference_world.hpp"
copy_header reference/world_derived.hpp "$source_dir/src/saveload/reference_world_derived.hpp"
copy_header reference/content.hpp "$source_dir/src/saveload/reference_content.hpp"
copy_header reference/terrain.hpp "$source_dir/src/saveload/reference_terrain.hpp"
copy_header reference/allocation.hpp "$source_dir/src/saveload/reference_allocation.hpp"
copy_header reference/runtime_road.hpp "$source_dir/src/saveload/reference_runtime_road.hpp"
copy_header reference/runtime_road_fixture.hpp "$source_dir/src/saveload/reference_runtime_road_fixture.hpp"
copy_header reference/runtime_road_fixture_tiles.hpp "$source_dir/src/saveload/reference_runtime_road_fixture_tiles.hpp"
copy_header reference/grf_scan.hpp "$source_dir/src/saveload/reference_grf_scan.hpp"
copy_header reference/grf_metadata.hpp "$source_dir/src/saveload/reference_grf_metadata.hpp"
copy_header reference/depot_runtime.hpp "$source_dir/src/saveload/reference_depot_runtime.hpp"
copy_header reference/world_fixture.hpp "$source_dir/src/saveload/reference_world_fixture.hpp"
copy_header reference/gameplay.hpp "$source_dir/src/saveload/reference_gameplay.hpp"
copy_header reference/callbacks.hpp "$source_dir/src/saveload/reference_callbacks.hpp"
for header in callback_vehicle callback_timer callback_house callback_company callback_station callback_industry callback_industry_state; do
    copy_header "reference/$header.hpp" "$source_dir/src/saveload/reference_$header.hpp"
done
cmake -S "$source_dir" -B "$build_dir" \
    -DOPTION_DEDICATED=ON -DOPTION_USE_ASSERTS=ON -DCMAKE_BUILD_TYPE=Release \
    "-DCMAKE_C_COMPILER_LAUNCHER=${REFERENCE_COMPILER_LAUNCHER:-}" \
    "-DCMAKE_CXX_COMPILER_LAUNCHER=${REFERENCE_COMPILER_LAUNCHER:-}" \
    "-DCMAKE_DISABLE_PRECOMPILE_HEADERS=${REFERENCE_DISABLE_PCH:-OFF}"
cmake --build "$build_dir" --parallel "${JOBS:-4}"
cmake -E sha256sum "$build_dir/openttd" reference/*.patch reference/*.hpp > "$build_dir/replay-build.sha256"
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
