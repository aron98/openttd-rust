#!/bin/sh
# Bounded variadic proof; undefined original executions are never in this corpus.
set -eu
if [ "$#" -ne 2 ]; then echo 'usage: verify.sh PINNED_OPENTTD OUTPUT' >&2; exit 64; fi
checkout=$1
out=$2
test ! -e "$out"
mkdir -p "$out"
out=$(CDPATH= cd -- "$out" && pwd)
root=$(CDPATH= cd -- "$(dirname "$0")/../../../.." && pwd)
cd "$root"
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-"$out/target"}
git rev-parse HEAD > "$out/revision.txt"
git status --porcelain > "$out/git-status.txt"
rustc -vV > "$out/rust-version.txt"
c++ --version > "$out/cxx-version.txt"
scripts/compat/script-vm/varargs/build-native.sh "$checkout" "$out/native" > "$out/native-build.log" 2>&1
python3 -m scripts.compat.script-vm.varargs.owners "$out/native/observe" "$out/owners" > "$out/owners.log" 2>&1
export OTTD_SCRIPT_VARARGS_RETURN="$out/owners/owner-return.stdout"
export OTTD_SCRIPT_VARARGS_TAIL="$out/owners/owner-tail.stdout"
export OTTD_SCRIPT_VARARGS_ERROR="$out/owners/owner-error.stdout"
for profile in debug release; do
 flag=
 if [ "$profile" = release ]; then flag=--release; fi
 cargo build -vv -p ottd-script --example observe $flag > "$out/$profile-build.log" 2>&1
 python3 -m scripts.compat.script-vm.varargs.compare "$out/native/observe" "$CARGO_TARGET_DIR/$profile/examples/observe" "$out/$profile" > "$out/$profile-compare.log" 2>&1
 python3 -m scripts.compat.script-vm.varargs.audit_controls "$out/$profile" "$out/$profile-controls" > "$out/$profile-controls.log" 2>&1
 OTTD_SCRIPT_VARARGS_TRACE=1 python3 -m scripts.compat.script-vm.tables.closures "$out/native/observe" "$CARGO_TARGET_DIR/$profile/examples/observe" "$out/$profile-closures" > "$out/$profile-closures.log" 2>&1
 cargo test -p ottd-script $flag --message-format=json > "$out/$profile-tests.jsonl" 2> "$out/$profile-tests.stderr"
 mkdir "$out/$profile-tests"
 find "$CARGO_TARGET_DIR/$profile/deps" -maxdepth 1 -type f -perm -100 ! -name '*.dylib' ! -name '*.so' -exec cp {} "$out/$profile-tests/" \;
done
cargo fmt --all -- --check > "$out/fmt.log" 2>&1
cargo clippy -p ottd-script --all-targets --all-features -- -D warnings > "$out/clippy.log" 2>&1
printf 'PASS bounded variadic compiler, frames, owners, defined errors; debug and release\n' > "$out/result.txt"
