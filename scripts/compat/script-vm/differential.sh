#!/bin/sh
# Run both actual executables; preserve every source and stdout/stderr separately.
set -eu
if [ "$#" -ne 3 ]; then echo 'usage: differential.sh NATIVE RUST OUTPUT' >&2; exit 64; fi
native=$1
rust=$2
out=$3
fixtures=$(CDPATH= cd -- "$(dirname "$0")/fixtures" && pwd)
root=$(CDPATH= cd -- "$(dirname "$0")/../../.." && pwd)
export PYTHONPATH="$root${PYTHONPATH:+:$PYTHONPATH}"
mkdir -p "$out"
: > "$out/invocations.txt"
for source in "$fixtures"/*.nut; do
  name=$(basename "$source" .nut)
  cp "$source" "$out/$name.nut"
  for credits in '10000' '0 1 2 3 100' '1 1 1 2 2 2 2 100' '2' '3' '4' '8' '9' '10'; do
    label=$(printf '%s' "$credits" | tr ' ' '-')
    prefix="$out/$name.$label"
    printf '%s %s %s\n%s %s %s\n' "$native" "$source" "$credits" "$rust" "$source" "$credits" >> "$out/invocations.txt"
    # Intentional word splitting: credits is a fixed, numeric list above.
    native_status=0
    "$native" "$source" $credits > "$prefix.native.stdout" 2> "$prefix.native.stderr" || native_status=$?
    rust_status=0
    "$rust" "$source" $credits > "$prefix.rust.stdout" 2> "$prefix.rust.stderr" || rust_status=$?
    printf 'native %s\nrust %s\n' "$native_status" "$rust_status" > "$prefix.status"
    python3 -m scripts.script_vm_policy "$source" "$prefix" "$native_status" "$rust_status"
    printf 'PASS %s %s\n' "$name" "$credits"
  done
done
