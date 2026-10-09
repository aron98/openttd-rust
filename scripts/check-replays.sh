#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
oracle="${OTTD_REPLAY_ORACLE:-$root/.reference/snapshot-build/openttd}"
original="${OTTD_REPLAY_ORIGINAL_ORACLE:-$root/.reference/build/openttd}"
test -x "$oracle"
test -x "$original"
cmake -DORACLE="$oracle" -P scripts/check-replay-build.cmake
if [ -n "${OTTD_REPLAY_CLI:-}" ]; then
    cli="$OTTD_REPLAY_CLI"
else
    cargo build --locked -p ottd-cli
    cli="$root/target/debug/ottd"
fi
if [ -n "${OTTD_REPLAY_ARTIFACTS:-}" ]; then
    run="$OTTD_REPLAY_ARTIFACTS"
    announced="$run"
else
    mkdir -p "$root/.artifacts"
    parent="$(mktemp -d "$root/.artifacts/replays-XXXXXX")"
    run="$parent/results"
    announced="$parent"
fi
printf 'Artifacts: %s\n' "$announced"
python3 -m scripts.check-replay --oracle "$oracle" --original "$original" --ottd "$cli" --artifacts "$run"
printf 'PASS replay checks\n'
