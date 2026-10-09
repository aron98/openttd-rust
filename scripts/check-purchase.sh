#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
oracle="${OTTD_PURCHASE_ORACLE:-$root/.reference/snapshot-build/openttd}"
test -x "$oracle"
cargo build --locked -p ottd-cli
cli="${OTTD_PURCHASE_CLI:-${CARGO_TARGET_DIR:-$root/target}/debug/ottd}"
test -x "$cli"
mkdir -p "$root/.artifacts"
parent="$(mktemp -d "$root/.artifacts/purchase-XXXXXX")"
printf 'Artifacts: %s\n' "$parent"
python3 scripts/purchase_provenance.py "$parent" "$oracle"
shasum -a 256 Cargo.toml Cargo.lock scripts/check-purchase.sh scripts/purchase_evidence.py \
    scripts/purchase-evidence-layout.json scripts/purchase_matrix.py scripts/purchase_replay.py \
    scripts/purchase_controls.py scripts/purchase_provenance.py \
    scripts/purchase_creation.py scripts/purchase_saved_state.py \
    scripts/purchase_duration_controls.py scripts/replay_matrix.py \
    crates/ottd-sim/src/commands.rs crates/ottd-sim/src/commands/pipeline.rs \
    crates/ottd-sim/src/lib.rs reference/replay_commands.hpp reference/replay_cost.hpp \
    reference/runtime_road.hpp reference/replay_hooks.hpp \
    crates/ottd-sim/src/runtime.rs crates/ottd-sim/src/runtime/*.rs \
    crates/ottd-sim/src/runtime/road_record/*.rs crates/ottd-sim/src/commands/vehicle_build.rs \
    crates/ottd-sim/src/commands/vehicle_build/*.rs crates/ottd-sim/tests/native_purchase.rs \
    scripts/setup-snapshot-reference.sh scripts/reference.cfg \
    fixtures/replay/clear-v362.sav "$oracle" "$cli" > "$parent/source-sha256.txt"
python3 -m scripts.purchase_matrix --oracle "$oracle" --ottd "$cli" --artifacts "$parent/results"
python3 scripts/purchase_evidence.py "$parent"
printf 'PASS owned-runtime road purchases\n' | tee "$parent/summary.txt"
