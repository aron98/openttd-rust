#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
oracle="${OTTD_GAMEPLAY_ORACLE:-$root/.reference/snapshot-build/openttd}"
runner="${OTTD_GAMEPLAY_RUNNER:-$root/scripts/run-gameplay-reference.cmake}"
test -x "$oracle"
test -f "$runner"
mkdir -p .artifacts
run="$(mktemp -d "$root/.artifacts/simulation-XXXXXX")"
printf 'Artifacts: %s\n' "$run"
cmake "-DORACLE=$oracle" "-DRUN_DIR=$run/native" \
    "-DCONFIG=$root/scripts/reference.cfg" "-DINPUT=$root/fixtures/generated-v362.sav" \
    -P "$runner" > "$run/native-run.log" 2>&1
export OTTD_GAMEPLAY_JSON="$run/native/gameplay.json"
export OTTD_SIMULATION_DIR="$run/landscape"
cargo test --locked -p ottd-core --test clock_oracle > "$run/clocks.log" 2>&1
cargo test --locked -p ottd-cli --test simulation -- --ignored > "$run/landscape.log" 2>&1
printf 'PASS: native clock cases, exact terrain checkpoints, repeated output and five negative controls\n'
