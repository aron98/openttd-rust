#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
oracle="$root/.reference/build/openttd"
test -x "$oracle"
commit="$(sed -n 's/^commit = "\([^"]*\)"/\1/p' upstream.toml)"
test "$(git -C .reference/OpenTTD rev-parse HEAD)" = "$commit"
git -C .reference/OpenTTD diff --exit-code HEAD
cargo build --locked -p ottd-cli
cli="$root/target/debug/ottd"
mkdir -p .artifacts
run="$(mktemp -d "$root/.artifacts/compat-XXXXXX")"
printf 'Artifacts: %s\n' "$run"

reference_run() {
    cmake "-DORACLE=$oracle" "-DRUN_DIR=$1" \
        "-DCONFIG=$root/scripts/reference.cfg" "-DINPUT=$2" -DTICKS=64 \
        -P "$root/scripts/run-reference.cmake"
}

printf 'OTTN\001\152\000\000TEST\000\000\000\000\000\000\000\000' > "$run/invalid.sav"
if reference_run "$run/invalid" "$run/invalid.sav" > "$run/negative-check.log" 2>&1; then
    printf 'FAIL: reference harness accepted an invalid game\n' >&2
    exit 1
fi
grep -F 'Reference engine reported a save/load error' "$run/negative-check.log"
printf 'PASS: reference harness rejects a failed load despite upstream exit status zero\n'

for fixture in "$root"/fixtures/*.sav; do
    name="$(basename "$fixture" .sav)"
    case_dir="$run/$name"
    mkdir -p "$case_dir"
    magic="$(dd if="$fixture" bs=1 count=4 2>/dev/null)"
    case "$magic" in
        OTTN) cp "$fixture" "$case_dir/independent-none.sav" ;;
        OTTX)
            {
                printf 'OTTN'
                dd if="$fixture" bs=1 skip=4 count=4 2>/dev/null
                dd if="$fixture" bs=1 skip=8 2>/dev/null | xz --decompress
            } > "$case_dir/independent-none.sav"
            ;;
        *) printf 'No independent fixture decoder for %s\n' "$magic" >&2; exit 1 ;;
    esac
    "$cli" rewrite "$fixture" "$case_dir/original-none.sav" --compression none
    cmp "$case_dir/independent-none.sav" "$case_dir/original-none.sav"
    reference_run "$case_dir/import" "$fixture"
    native="$case_dir/import/save/autosave/exit.sav"
    reference_run "$case_dir/baseline" "$native"
    for compression in none zlib lzma lzo; do
        rewritten="$case_dir/$compression.sav"
        "$cli" rewrite "$fixture" "$rewritten" --compression "$compression"
        "$cli" rewrite "$rewritten" "$case_dir/$compression-decoded.sav" --compression none
        cmp "$case_dir/original-none.sav" "$case_dir/$compression-decoded.sav"
        reference_run "$case_dir/import-$compression" "$rewritten"
        "$cli" rewrite "$native" "$case_dir/native-$compression.sav" --compression "$compression"
        reference_run "$case_dir/$compression" "$case_dir/native-$compression.sav"
        cmp "$case_dir/baseline/save/autosave/exit.sav" \
            "$case_dir/$compression/save/autosave/exit.sav"
        printf 'PASS %s / %s: original payload preserved and loaded; shared native state identical after 64 ticks\n' "$name" "$compression"
    done
done
