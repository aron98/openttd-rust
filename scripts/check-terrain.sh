#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
oracle="${OTTD_TERRAIN_ORACLE:-$root/.reference/snapshot-build/openttd}"
test -x "$oracle"
if [ -n "${OTTD_TERRAIN_ARTIFACTS:-}" ]; then
    run="$OTTD_TERRAIN_ARTIFACTS"
    test ! -e "$run"
    mkdir -p "$run"
else
    mkdir -p .omo/evidence
    run="$(mktemp -d "$root/.omo/evidence/stage04-terrain-XXXXXX")"
fi
printf 'Artifacts: %s\n' "$run"
git rev-parse HEAD > "$run/rust-commit.txt"
git -C .reference/OpenTTD rev-parse HEAD > "$run/native-commit.txt"
shasum -a 256 crates/ottd-core/src/terrain.rs crates/ottd-core/tests/terrain_oracle.rs crates/ottd-sim/src/terrain.rs crates/ottd-sim/tests/terrain.rs reference/terrain.hpp scripts/run-terrain-reference.cmake scripts/check-terrain.sh > "$run/source-sha256.txt"
export OTTD_TERRAIN_FIXTURE_DIR="$run/fixtures"
cargo test --locked -p ottd-sim --test terrain prepare_native_terrain_worlds -- --ignored --exact > "$run/fixtures.log" 2>&1
grep -E -q '^test prepare_native_terrain_worlds \.\.\. ok$' "$run/fixtures.log"
grep -E -q '^test result: ok\. 1 passed; 0 failed; 0 ignored;' "$run/fixtures.log"
for freeform in 0 1; do
    export OTTD_TERRAIN_SAVE="$run/fixtures/freeform-$freeform.sav"
    cmake "-DORACLE=$oracle" "-DRUN_DIR=$run/native-$freeform" "-DCONFIG=$root/scripts/reference.cfg" "-DINPUT=$OTTD_TERRAIN_SAVE" -P scripts/run-terrain-reference.cmake > "$run/native-$freeform.log" 2>&1
    export OTTD_TERRAIN_JSON="$run/native-$freeform/terrain.json"
    cargo test --locked -p ottd-core --test terrain_oracle terrain_matches_native_vectors -- --ignored --exact > "$run/core-$freeform.log" 2>&1
    grep -E -q '^test terrain_matches_native_vectors \.\.\. ok$' "$run/core-$freeform.log"
    grep -E -q '^test result: ok\. 1 passed; 0 failed; 0 ignored;' "$run/core-$freeform.log"
    cargo test --locked -p ottd-sim --test terrain world_geometry_matches_native_vectors -- --ignored --exact > "$run/world-$freeform.log" 2>&1
    grep -E -q '^test world_geometry_matches_native_vectors \.\.\. ok$' "$run/world-$freeform.log"
    grep -E -q '^test result: ok\. 1 passed; 0 failed; 0 ignored;' "$run/world-$freeform.log"
done
export OTTD_TERRAIN_JSON="$run/native-0/negative.json"
if cargo test --locked -p ottd-core --test terrain_oracle terrain_matches_native_vectors -- --ignored --exact > "$run/negative.log" 2>&1; then
    printf 'FAIL: altered native pixel was accepted\n' >&2
    exit 1
fi
grep -E -q 'slope=0 pixel=0' "$run/negative.log"
grep -E -q '^test terrain_matches_native_vectors \.\.\. FAILED$' "$run/negative.log"
grep -E -q '^test result: FAILED\. 0 passed; 1 failed; 0 ignored;' "$run/negative.log"
cargo test --locked -p ottd-core --test terrain_oracle > "$run/core-boundaries.log" 2>&1
cargo test --locked -p ottd-sim --test terrain > "$run/world-boundaries.log" 2>&1
printf 'PASS: native terrain, both edge modes, checked boundaries and altered-vector rejection\n' | tee "$run/summary.txt"
