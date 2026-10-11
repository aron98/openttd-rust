# Bundled Squirrel 2.2 variadic consumer

`verify.sh PINNED_OPENTTD OUTPUT` builds the unchanged passive v09 observer from
OpenTTD `14ec60f248547d4d062a1160f0fc26d742319888`, compares complete prototype,
bytecode, frame, vararg-stack, budget, terminal and raw print records, and feeds
three original owner sessions into Rust tests. It retains binaries, source
fixtures, actual argv/status/output receipts and independent read-back checks.
The output directory must be fresh. Run from the repository root with an absolute
pinned checkout path. Debug and release each execute 48 sources across three
budget schedules; the deep-stack source appends a 100000 credit to each schedule.
The previous closure corpus is rerun with the same passive vararg tracing.

The native observer sources here are byte-identical to the approved S7 design's
v09 observer; no compiler or VM code is patched. Eight preimplementation tests
were genuinely RED at the base revision before production changes. Additional
Rust tests cover reached rejection, earlier side effects, reuse, suspended drop,
checked operands, nonfinite index policy and deep native-width lookup semantics.

`vargv` is an indexed opcode, not a materialized array. Each frame owns unnamed
values separately from fixed registers. The native unsigned-short lookup base
wraps at 65536; physical cleanup still pops the actual count. The deep source has
331 frames of 200 extras: base66000 becomes464 and accesses an existing retained
value329. No invalid memory access is required or executed by this test.

The only new undefined-original refusal is a reached nongenerator script-closure
tail call with nonzero outgoing extras and invalid callee arity. The original
pops those extras before validating arity and later consumes the stale count.
Rust reports `UndefinedNativeVarargsTailCall` before destructive replacement,
after previous body and argument effects. Tests for this refusal are Rust-only;
this producer never executes those undefined sources on the original. Ordinary
wrong arity, zero-extra tail errors, native/noncallable errors and post-replacement
errors retain their defined behavior. Nonfinite/out-of-range index conversion
uses the existing `UnsupportedIndexConversion` policy only after the empty-extra
check. The u8 call count includes the receiver, bounding per-frame extras to254;
invalid bytecode is rejected separately.

Compiler diagnostics remain typed Rust diagnostics; compiler-error cases compare
terminal stage while pinning the original diagnostic payload. Executable cases
compare complete stdout, raw prints and runtime-error bytes. Corruption controls
mutate only private copies, verifying missing receipts, changed source, changed
output and malformed receipt rejection before restoring the copied baseline.

This is a bounded language consumer. General GC, traps, generators, classes,
host API integration, GNU-platform proof and Stage4 completion are not claimed.
Shared capability manifests and current source bindings remain leader-owned.
