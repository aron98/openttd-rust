#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
evidence="${1:?pass a fresh evidence directory}"
oracle="${2:?pass the freshly built rail observer executable}"
test ! -e "$evidence"
mkdir -p "$evidence"
evidence="$(cd "$evidence" && pwd)"
cd "$root"
exec > "$evidence/invocations.log" 2>&1
set -x
for engine in 0 8 13; do
    directory="$evidence/engine-$engine"
    cmake "-DORACLE=$oracle" "-DRUN_DIR=$directory/prepare" \
        "-DCONFIG=$root/scripts/reference.cfg" "-DINPUT=$root/fixtures/replay/clear-v362.sav" \
        "-DENGINE=$engine" -P "$root/scripts/check-rail-restore-reference.cmake"
    cmake "-DORACLE=$oracle" "-DRUN_DIR=$directory/observe" \
        "-DCONFIG=$root/scripts/reference.cfg" "-DINPUT=$directory/prepare/save/autosave/exit.sav" \
        -P "$root/scripts/check-rail-restore-reference.cmake"
    OTTD_RAIL_EVIDENCE="$directory" OTTD_RAIL_ORACLE="$oracle" \
        cargo test -p ottd-sim --test native_rail_restoration -- --ignored --nocapture
    if [ "$engine" = 0 ]; then
        OTTD_RAIL_EVIDENCE="$directory" OTTD_RAIL_ORACLE="$oracle" \
            cargo test -p ottd-sim --test native_rail_controls -- --ignored --nocapture
    fi
done
printf '%s\n' 'PASS three authentic original engines; saved mutation controls; typed refusals; observer neutrality; both save-origin load continuations' > "$evidence/result.txt"
