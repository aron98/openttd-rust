#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
oracle="${OTTD_SERVICE_ORACLE:-$root/.reference/snapshot-build/openttd}"
test -x "$oracle"
if [ -n "${OTTD_SERVICE_CLI:-}" ]; then
    cli="$OTTD_SERVICE_CLI"
else
    cargo build --locked -p ottd-cli
    cli="${CARGO_TARGET_DIR:-$root/target}/debug/ottd"
fi
test -x "$cli"
mkdir -p "$root/.artifacts"
parent="$(mktemp -d "$root/.artifacts/service-XXXXXX")"
printf 'Artifacts: %s\n' "$parent"
python3 -m scripts.service_replay --oracle "$oracle" --ottd "$cli" --artifacts "$parent/results"
printf 'PASS native service replays\n'
