#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
oracle="${OTTD_WORLD_ORACLE:-$root/.reference/snapshot-build/openttd}"
original="${OTTD_WORLD_ORIGINAL_ORACLE:-$root/.reference/build/openttd}"
test -x "$oracle"
test -x "$original"
if [ -n "${OTTD_WORLD_CLI:-}" ]; then
    cli="$OTTD_WORLD_CLI"
else
    cargo build --locked -p ottd-cli
    cli="$root/target/debug/ottd"
fi
if [ -n "${OTTD_WORLD_ARTIFACTS:-}" ]; then
    run="$OTTD_WORLD_ARTIFACTS"
else
    mkdir -p "$root/.artifacts"
    parent="$(mktemp -d "$root/.artifacts/worlds-XXXXXX")"
    run="$parent/results"
    printf 'Artifacts: %s\n' "$parent"
fi
python3 -m scripts.check-world --oracle "$oracle" --original "$original" --ottd "$cli" --artifacts "$run"
