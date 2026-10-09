#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
oracle="${OTTD_GRF_METADATA_ORACLE:-$root/.reference/snapshot-build/openttd}"
test -x "$oracle"
mkdir -p "$root/.artifacts"
parent="$(mktemp -d "$root/.artifacts/grf-metadata-XXXXXX")"
printf 'Artifacts: %s\n' "$parent"
export OTTD_GRF_METADATA_ORACLE="$oracle"
export OTTD_GRF_METADATA_DIR="$parent/results"
shasum -a 256 Cargo.toml Cargo.lock crates/ottd-sim/Cargo.toml \
    crates/ottd-sim/src/content/grf.rs crates/ottd-sim/src/content/grf/*.rs \
    crates/ottd-sim/tests/native_grf_metadata.rs crates/ottd-sim/tests/grf_metadata.rs \
    crates/ottd-sim/tests/grf_metadata_cases/*.rs reference/grf_metadata.hpp \
    reference/world_derived.hpp scripts/check-grf-metadata-reference.cmake \
    scripts/check-replay-build.cmake scripts/setup-snapshot-reference.sh \
    scripts/check-grf-metadata.sh scripts/grf_metadata_evidence.py scripts/reference.cfg fixtures/replay/clear-v362.sav \
    "$oracle" > "$parent/source-sha256.txt"
cargo test --locked -p ottd-sim --test native_grf_metadata native_metadata_matrix -- --ignored --exact \
    > "$parent/native-test.log" 2>&1
rg -q '^test native_metadata_matrix \.\.\. ok$' "$parent/native-test.log"
rg -q 'test result: ok\. 1 passed; 0 failed; 0 ignored;' "$parent/native-test.log"
cargo test --locked -p ottd-sim --test grf_metadata host_bounds_are_cumulative_and_distinct_from_native_disabling -- --exact \
    > "$parent/host-boundaries.log" 2>&1
rg -q '^test host_bounds_are_cumulative_and_distinct_from_native_disabling \.\.\. ok$' "$parent/host-boundaries.log"
rg -q 'test result: ok\. 1 passed; 0 failed; 0 ignored;' "$parent/host-boundaries.log"
python3 scripts/grf_metadata_evidence.py "$parent"
printf 'PASS fresh-config GRF metadata and host bounds\n' | tee "$parent/summary.txt"
