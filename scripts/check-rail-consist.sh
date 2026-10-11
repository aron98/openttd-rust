#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
proof="${1:?pass a fresh consist proof directory}"
export OTTD_CONSIST_ORACLE="${2:?pass the original consist observer executable}"
test ! -e "$proof"
mkdir -p "$proof"
proof="$(cd "$proof" && pwd)"
cd "$root"
python3 -m scripts.rail_sources "$root" "$proof/source-closure"
exec > "$proof/invocations.log" 2>&1
set -x
for roster in 0,27,29 13,28,34 11,27 11,11,27,29 0,13,27,29; do
    export OTTD_CONSIST_EVIDENCE="$proof/$roster"
    mkdir "$OTTD_CONSIST_EVIDENCE"
    for phase in prepare observe; do
        input="$root/fixtures/replay/clear-v362.sav"
        argument="-DPASSIVE_CONTROL=false"
        if [ "$phase" = prepare ]; then
            argument="-DENGINE=$roster"
        else
            input="$OTTD_CONSIST_EVIDENCE/prepare/save/autosave/exit.sav"
        fi
        cmake "-DORACLE=$OTTD_CONSIST_ORACLE" "-DRUN_DIR=$OTTD_CONSIST_EVIDENCE/$phase" \
            "-DCONFIG=$root/scripts/reference.cfg" "-DINPUT=$input" "$argument" \
            -P scripts/check-rail-consist-reference.cmake > "$OTTD_CONSIST_EVIDENCE/$phase.launch.log" 2>&1
    done
    cargo test --locked -p ottd-sim --test native_rail_consist_design \
        native_both_save_origins_and_observer_neutrality -- --ignored --exact --nocapture \
        > "$OTTD_CONSIST_EVIDENCE/native-control.log" 2>&1
    for scenario in original_origin_consist_restoration rust_origin_consist_restoration; do
        set +e
        cargo test --locked -p ottd-sim --test native_rail_consist_design "$scenario" \
            -- --ignored --exact --nocapture > "$OTTD_CONSIST_EVIDENCE/$scenario.log" 2>&1
        code=$?
        set -e
        printf '%s\n' "$code" > "$OTTD_CONSIST_EVIDENCE/$scenario.exit"
        test "$code" -eq 0
        rg -q 'test result: ok' \
            "$OTTD_CONSIST_EVIDENCE/$scenario.log"
    done
done
export OTTD_CONSIST_EVIDENCE="$proof/0,27,29"
cargo test --locked -p ottd-sim --test native_rail_consist_controls rust_only_invalid_chain_controls \
    -- --ignored --exact --nocapture > "$proof/rust-only-controls.log" 2>&1
for scenario in stale_acceleration powered_flags saved_capacity coupled_saved_mutations suffix_occupancy; do
    set +e
    cargo test --locked -p ottd-sim --test native_rail_consist_controls "$scenario" \
        -- --ignored --exact --nocapture > "$proof/$scenario.log" 2>&1
    code=$?
    set -e
    printf '%s\n' "$code" > "$proof/$scenario.exit"
    test "$code" -eq 0
    rg -q 'test result: ok' "$proof/$scenario.log"
done
printf '%s\n' 'PASS fourteen former REDs and added per-part suffix-occupancy witness; both save origins, complete caches/World/RNG/neutrality and typed refusal controls' > "$proof/result.txt"
for scenario in late_part_refusals_roll_back_staged_records multihead_orders_and_multiple_roots_are_specific_refusals; do
    cargo test --locked -p ottd-sim --test native_rail_consist_refusals "$scenario" \
        -- --ignored --exact --nocapture > "$proof/$scenario.log" 2>&1
done
OTTD_CONSIST_EVIDENCE="$proof/11,11,27,29" cargo test --locked -p ottd-sim \
    --test native_rail_consist_refusals rejects_native_multihead_repair_before_publication \
    -- --ignored --exact --nocapture > "$proof/multihead-repair.log" 2>&1
printf '%s\n' 'PASS typed late-part rollback after two actual staged records, multihead repair/orders/electric/multiple-root refusals; no invalid inputs executed natively' >> "$proof/result.txt"
cargo test --locked -p ottd-sim --test native_rail_consist_context -- --ignored --nocapture \
    > "$proof/context-refusals.log" 2>&1
printf '%s\n' 'PASS valid-different-owner, NewGRF-content and populated-cargo typed refusals with unchanged whole world; Rust only' >> "$proof/result.txt"
