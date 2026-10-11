#!/bin/sh
# Fresh target-native proof; no saved macOS ordering is substituted on Linux.
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
export OTTD_SCRIPT_TABLE_HASHES="$out/hashes.stdout"
export OTTD_SCRIPT_TABLE_OWNER="$out/owner.stdout"
export OTTD_SCRIPT_TABLE_MUTATION="$out/mutation.stdout"
export OTTD_SCRIPT_CLONE_OWNER="$out/clone-owner.stdout"
export OTTD_SCRIPT_DELETE_OWNER="$out/delete-owner.stdout"
export OTTD_SCRIPT_DELETE_MUTATION="$out/delete-mutation.stdout"
export OTTD_SCRIPT_TABLE_STORAGE="$out/table-storage.stdout"
export OTTD_SCRIPT_DELETE_EFFECTS="$out/delete-effects.stdout"
export OTTD_SCRIPT_GRAPH_STORAGE="$out/graph-storage.stdout"
export OTTD_SCRIPT_GRAPH_VM="$out/graph-vm.stdout"
export OTTD_SCRIPT_GRAPH_TEARDOWN="$out/graph-teardown.stdout"
c++ --version > "$out/cxx-version.txt"
rustc -vV > "$out/rust-version.txt"
git rev-parse HEAD > "$out/base-revision.txt"
scripts/compat/script-vm/build-native.sh "$checkout" "$out/native" > "$out/native-build.log" 2>&1
"$out/native/observe" --table-hashes > "$out/hashes.stdout" 2> "$out/hashes.stderr"
"$out/native/observe" --table-owner scripts/compat/script-vm/tables/fixtures/owner.nut > "$out/owner.stdout" 2> "$out/owner.stderr"
"$out/native/observe" --table-mutation scripts/compat/script-vm/tables/fixtures/mutation.nut > "$out/mutation.stdout" 2> "$out/mutation.stderr"
for name in clone-owner delete-owner; do
 "$out/native/observe" --table-owner "scripts/compat/script-vm/tables/fixtures/$name.nut" > "$out/$name.stdout" 2> "$out/$name.stderr"
done
"$out/native/observe" --table-removal scripts/compat/script-vm/tables/fixtures/delete-mutation.nut > "$out/delete-mutation.stdout" 2> "$out/delete-mutation.stderr"
"$out/native/observe" --table-storage > "$out/table-storage.stdout" 2> "$out/table-storage.stderr"
"$out/native/observe" --table-effects scripts/compat/script-vm/tables/fixtures/delete-effects.nut > "$out/delete-effects.stdout" 2> "$out/delete-effects.stderr"
"$out/native/observe" --graph-storage > "$out/graph-storage.stdout" 2> "$out/graph-storage.stderr"
"$out/native/observe" --graph-vm scripts/compat/script-vm/tables/fixtures/graph/suspended.nut scripts/compat/script-vm/tables/fixtures/graph/late-error.nut > "$out/graph-vm.stdout" 2> "$out/graph-vm.stderr"
"$out/native/observe" --graph-teardown > "$out/graph-teardown.stdout" 2> "$out/graph-teardown.stderr"
for repeat in 1 2 3; do
 "$out/native/observe" --table-pointer-keys > "$out/pointer-$repeat.stdout" 2> "$out/pointer-$repeat.stderr"
done
for profile in debug release; do
 flag=
 if [ "$profile" = release ]; then flag=--release; fi
 cargo build -p ottd-script --example observe $flag > "$out/$profile-build.log" 2>&1
 python3 -m scripts.compat.script-vm.tables.compare "$out/native/observe" "$CARGO_TARGET_DIR/$profile/examples/observe" "$out/$profile" > "$out/$profile-compare.log" 2>&1
 python3 -m scripts.compat.script-vm.tables.controls "$out/$profile" > "$out/$profile-controls.log" 2>&1
 cargo test -p ottd-script $flag > "$out/$profile-tests.log" 2>&1
 cargo test -p ottd-script $flag native_string_hashes -- --ignored > "$out/$profile-hashes.log" 2>&1
 # Freeze all executable test artifacts, which otherwise drift in the shared cache.
 mkdir "$out/$profile-tests"
 find "$CARGO_TARGET_DIR/$profile/deps" -maxdepth 1 -type f -perm -100 ! -name '*.dylib' ! -name '*.so' -exec cp {} "$out/$profile-tests/" \;
done
cargo fmt --all -- --check > "$out/fmt.log" 2>&1
cargo clippy -p ottd-script --all-targets -- -D warnings > "$out/clippy.log" 2>&1
printf 'PASS native hashes, tables, lifetime, suspended mutation; debug and release\n' > "$out/result.txt"
