#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
oracle="${OTTD_SNAPSHOT_ORACLE:-$root/.reference/snapshot-build/openttd}"
runner="${OTTD_SNAPSHOT_RUNNER:-$root/scripts/run-snapshot-reference.cmake}"
test -x "$oracle"
test -f "$runner"
cargo build --locked -p ottd-cli
cli="$root/target/debug/ottd"
mkdir -p .artifacts
run="$(mktemp -d "$root/.artifacts/snapshots-XXXXXX")"
printf 'Artifacts: %s\n' "$run"

for fixture in "$root"/fixtures/*.sav; do
    name="$(basename "$fixture" .sav)"
    for ticks in 1 16; do
        case_dir="$run/$name-$ticks"
        cmake "-DORACLE=$oracle" "-DRUN_DIR=$case_dir" \
            "-DCONFIG=$root/scripts/reference.cfg" "-DINPUT=$fixture" "-DTICKS=$ticks" \
            -P "$runner"
        native="$case_dir/save/autosave/exit.sav"
        "$cli" snapshot "$native" > "$case_dir/rust.json"
        "$cli" snapshot "$native" > "$case_dir/rust-repeat.json"
        cmp "$case_dir/rust.json" "$case_dir/rust-repeat.json"
        "$cli" compare "$case_dir/snapshot.json" "$case_dir/rust.json" > "$case_dir/compare.log"
        OTTD_ORACLE_SAVE="$native" OTTD_ORACLE_JSON="$case_dir/snapshot.json" \
            cargo test --locked -p ottd-save --test oracle_parity -- --ignored \
            > "$case_dir/typed-parity.log" 2>&1
        OTTD_PRIMITIVES_JSON="$case_dir/primitives.json" cargo test --locked -p ottd-core \
            --test oracle > "$case_dir/primitives.log" 2>&1
        OTTD_SNAPSHOT_DIR="$case_dir" cargo test --locked -p ottd-cli \
            --test snapshot_controls -- --ignored > "$case_dir/controls.log" 2>&1
        printf 'PASS: %s at %s ticks, exact snapshot and four negative controls\n' "$name" "$ticks"
    done
done
