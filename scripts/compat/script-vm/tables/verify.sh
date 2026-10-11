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
export OTTD_SCRIPT_CLOSURE_COMPOUND_STORE="$out/closure-compound-store.stdout"
export OTTD_SCRIPT_CLOSURE_PUBLICATION="$out/closure-publication.stdout"
export OTTD_SCRIPT_CLOSURE_ACYCLIC="$out/closure-acyclic.stdout"
export OTTD_SCRIPT_CLOSURE_CYCLE="$out/closure-cycle.stdout"
export OTTD_SCRIPT_CLOSURE_ORPHAN="$out/closure-orphan.stdout"
export OTTD_SCRIPT_CLOSURE_VIEW="$out/closure-view.stdout"
export OTTD_SCRIPT_CLOSURE_DEFAULT_CYCLE="$out/closure-default-cycle.stdout"
export OTTD_SCRIPT_CLOSURE_PERSISTENCE="$out/closure-persistence.stdout"
export OTTD_SCRIPT_CLOSURE_LATE_ERROR="$out/closure-late-error.stdout"
export OTTD_SCRIPT_CALL_CALLEE="$out/call-callee.stdout"
export OTTD_SCRIPT_CALL_EMPTY="$out/call-empty.stdout"
export OTTD_SCRIPT_CALL_RETAINED="$out/call-retained.stdout"
export OTTD_SCRIPT_CALL_ARRAY_EMPTY="$out/call-array-empty.stdout"
export OTTD_SCRIPT_CALL_ARRAY_RETAINED="$out/call-array-retained.stdout"
export OTTD_SCRIPT_CALL_FAILURE="$out/call-failure.stdout"
export OTTD_SCRIPT_CALL_CYCLE="$out/call-cycle.stdout"
export OTTD_SCRIPT_TABLE_HASHES="$out/hashes.stdout"
export OTTD_SCRIPT_TABLE_OWNER="$out/owner.stdout"
export OTTD_SCRIPT_TABLE_MUTATION="$out/mutation.stdout"
export OTTD_SCRIPT_CLONE_OWNER="$out/clone-owner.stdout"
export OTTD_SCRIPT_DELETE_OWNER="$out/delete-owner.stdout"
export OTTD_SCRIPT_DELETE_MUTATION="$out/delete-mutation.stdout"
export OTTD_SCRIPT_TABLE_STORAGE="$out/table-storage.stdout"
export OTTD_SCRIPT_DELETE_EFFECTS="$out/delete-effects.stdout"
export OTTD_SCRIPT_ROOT_OWNERSHIP_REVERSE="$out/root-ownership-reverse.stdout"
export OTTD_SCRIPT_ROOT_OWNERSHIP="$out/root-ownership.stdout"
export OTTD_SCRIPT_ROOT_TEARDOWN="$out/root-teardown.stdout"
export OTTD_SCRIPT_ROOT_REGISTRATION="$out/root-registration.stdout"
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
"$out/native/observe" --root-ownership scripts/compat/script-vm/tables/fixtures/roots/escaped.nut > "$out/root-ownership.stdout" 2> "$out/root-ownership.stderr"
"$out/native/observe" --root-ownership scripts/compat/script-vm/tables/fixtures/roots/escaped-reverse.nut reverse > "$out/root-ownership-reverse.stdout" 2> "$out/root-ownership-reverse.stderr"
"$out/native/observe" --root-teardown > "$out/root-teardown.stdout" 2> "$out/root-teardown.stderr"
"$out/native/observe" --root-registration > "$out/root-registration.stdout" 2> "$out/root-registration.stderr"
"$out/native/observe" --graph-storage > "$out/graph-storage.stdout" 2> "$out/graph-storage.stderr"
"$out/native/observe" --graph-vm scripts/compat/script-vm/tables/fixtures/graph/suspended.nut scripts/compat/script-vm/tables/fixtures/graph/late-error.nut > "$out/graph-vm.stdout" 2> "$out/graph-vm.stderr"
"$out/native/observe" --graph-teardown > "$out/graph-teardown.stdout" 2> "$out/graph-teardown.stderr"
for repeat in 1 2 3; do
 "$out/native/observe" --table-pointer-keys > "$out/pointer-$repeat.stdout" 2> "$out/pointer-$repeat.stderr"
done
for entry in empty retained; do
 "$out/native/observe" --call-entry "$entry" > "$out/call-$entry.stdout" 2> "$out/call-$entry.stderr"
 "$out/native/observe" --call-array "$entry" > "$out/call-array-$entry.stdout" 2> "$out/call-array-$entry.stderr"
done
"$out/native/observe" --call-failure scripts/compat/script-vm/tables/fixtures/calls/late-error.nut > "$out/call-failure.stdout" 2> "$out/call-failure.stderr"
"$out/native/observe" --call-cycle-teardown > "$out/call-cycle.stdout" 2> "$out/call-cycle.stderr"
"$out/native/observe" --call-callee > "$out/call-callee.stdout" 2> "$out/call-callee.stderr"
for session in acyclic cycle orphan view default-cycle persistence late-error publication compound-store; do
 "$out/native/observe" "--closure-$session" > "$out/closure-$session.stdout" 2> "$out/closure-$session.stderr"
done
for profile in debug release; do
 flag=
 if [ "$profile" = release ]; then flag=--release; fi
 cargo clean -p ottd-script $flag > "$out/$profile-clean.log" 2>&1
 printf "0\n" > "$out/$profile-clean.status"
 cargo build -vv -p ottd-script --example observe $flag --message-format=json > "$out/$profile-build.jsonl" 2> "$out/$profile-build.log"
 rust=$(python3 -c 'import sys; from pathlib import Path; from scripts.script_vm_provenance import select_executable; print(select_executable(Path(sys.argv[1]).read_text(), "observe", "example", False))' "$out/$profile-build.jsonl")
 printf '%s\n' "$rust" > "$out/$profile-selected-observer.txt"
 python3 -m scripts.compat.script-vm.tables.compare "$out/native/observe" "$rust" "$out/$profile" > "$out/$profile-compare.log" 2>&1
 python3 -m scripts.compat.script-vm.tables.controls "$out/$profile" > "$out/$profile-controls.log" 2>&1
 python3 -m scripts.compat.script-vm.tables.trap_controls "$out/$profile" > "$out/$profile-trap-controls.log" 2>&1
 python3 -m scripts.compat.script-vm.tables.calls "$out/native/observe" "$rust" "$out/$profile-calls" > "$out/$profile-calls.log" 2>&1
 python3 -m scripts.compat.script-vm.tables.call_controls "$out/native/observe" "$rust" "$out/$profile-call-controls" > "$out/$profile-call-controls.log" 2>&1
 python3 -m scripts.compat.script-vm.tables.closures "$out/native/observe" "$rust" "$out/$profile-closures" > "$out/$profile-closures.log" 2>&1
 python3 -m scripts.compat.script-vm.tables.closure_controls "$out/native/observe" "$rust" "$out/$profile-closure-controls" > "$out/$profile-closure-controls.log" 2>&1
 for trap_case in caught_owner caught_cycle_owner uncaught_array; do
  "$out/native/observe" --trap-error-owner "$out/$profile/trap-$trap_case.nut" > "$out/$profile-trap-$trap_case.stdout" 2> "$out/$profile-trap-$trap_case.stderr"
 done
 export OTTD_SCRIPT_TRAP_CAUGHT_OWNER="$out/$profile-trap-caught_owner.stdout"
 export OTTD_SCRIPT_TRAP_CYCLE_OWNER="$out/$profile-trap-caught_cycle_owner.stdout"
 export OTTD_SCRIPT_TRAP_UNCAUGHT_OWNER="$out/$profile-trap-uncaught_array.stdout"
 cargo test -p ottd-script $flag --message-format=json > "$out/$profile-tests.jsonl" 2> "$out/$profile-tests.log"
 cargo test -p ottd-script $flag native_root_registration -- --ignored > "$out/$profile-root-registration.log" 2>&1
 cargo test -p ottd-script $flag native_string_hashes -- --ignored > "$out/$profile-hashes.log" 2>&1
 # Freeze all executable test artifacts, which otherwise drift in the shared cache.
 mkdir "$out/$profile-tests"
 find "$CARGO_TARGET_DIR/$profile/deps" -maxdepth 1 -type f -perm -100 ! -name '*.dylib' ! -name '*.so' -exec cp {} "$out/$profile-tests/" \;
done
cargo fmt --all -- --check > "$out/fmt.log" 2>&1
cargo clippy -p ottd-script --all-targets -- -D warnings > "$out/clippy.log" 2>&1
printf 'PASS native hashes, tables, calls, raw print, lifetime, suspended mutation; debug and release\n' > "$out/result.txt"
