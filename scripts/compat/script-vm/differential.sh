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
for source in "$fixtures"/*.nut; do
  name=$(basename "$source" .nut)
  cp "$source" "$out/$name.nut"
  for credits in '10000' '0 1 2 3 100' '1 1 1 2 2 2 2 100' '2' '3' '4' '8' '9' '10'; do
    label=$(printf '%s' "$credits" | tr ' ' '-')
    prefix="$out/$name.$label"
    # Intentional word splitting: credits is a fixed, numeric list above.
    python3 -m scripts.script_vm_policy "$source" "$prefix" "$native" "$rust" $credits
    printf 'PASS %s %s\n' "$name" "$credits"
  done
done
