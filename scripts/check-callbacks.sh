#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
oracle="${OTTD_CALLBACK_ORACLE:-$root/.reference/snapshot-build/openttd}"
test -x "$oracle"
mkdir -p .artifacts
run="$(mktemp -d "$root/.artifacts/callbacks-XXXXXX")"
printf 'Artifacts: %s\n' "$run"
cmake "-DORACLE=$oracle" "-DRUN_DIR=$run/native" \
    "-DCONFIG=$root/scripts/reference.cfg" "-DINPUT=$root/fixtures/generated-v362.sav" \
    -P scripts/run-callback-reference.cmake > "$run/native-run.log" 2>&1
export OTTD_CALLBACK_JSON="$run/native/callbacks.json"
export OTTD_CALLBACK_ARTIFACTS="$run/cli"
cargo test --locked -p ottd-sim --test vehicle_oracle --test periodic_oracle --test industry_oracle > "$run/library.log" 2>&1
cargo test --locked -p ottd-cli --test callbacks --test callbacks_compare > "$run/cli.log" 2>&1
cmp reference/callbacks.json "$OTTD_CALLBACK_JSON"
printf 'PASS: original callback state, CLI repeated output, seven rejected requests, nine exact output-mismatch controls, reproducible corpus\n'
