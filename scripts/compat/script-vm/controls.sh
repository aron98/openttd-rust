#!/bin/sh
# Comparator controls: success requires every intentional observation change to fail.
set -eu
if [ "$#" -ne 1 ]; then echo 'usage: controls.sh DIFFERENTIAL_OUTPUT' >&2; exit 64; fi
out=$1
base="$out/precedence.0-1-2-3-100.native.stdout"
for control in value opcode ip debt; do
  changed="$out/control-$control.stdout"
  case "$control" in
    value) sed 's/integer 25/integer 26/' "$base" > "$changed" ;;
    opcode) sed 's/op 17 1 2 1 43/op 17 1 2 1 45/' "$base" > "$changed" ;;
    ip) sed 's/suspend 0 2/suspend 0 3/' "$base" > "$changed" ;;
    debt) sed 's/suspend -1 0/suspend 0 0/' "$base" > "$changed" ;;
  esac
  if diff -u "$base" "$changed" > "$out/control-$control.diff"; then
    echo "FAIL comparator accepted $control corruption" >&2; exit 1
  fi
  test -s "$out/control-$control.diff"
  printf 'PASS rejected %s corruption\n' "$control"
done
