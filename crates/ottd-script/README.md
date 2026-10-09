# Squirrel scalar execution foundation

This crate independently compiles a bounded subset of OpenTTD 15.3's bundled
Squirrel 2.2.5 and executes native register instructions. It has no C++ runtime
or native delegation dependency. It is not an AI/GS runtime and is not admitted
to game ticks.

Supported source consists of one `return` statement (optional semicolon), with
null, bool, i64 decimal/octal/hex literals, well-formed f32 decimal/scientific
literals, parentheses, unary `- ! ~`, and binary `+ - * / %`. Space, tab, carriage return and line feed
are accepted; comments, declarations, names, multiple statements, functions,
objects, comparison and other operators are unsupported. A newline immediately
after `return` ends that statement, as native does. Source is bounded to 64 KiB
and expression depth to 128; excess is a typed compiler error.

The compiler emits native opcode tuples and a literal pool, retaining encounter
order, temporary-register reuse, DLOAD and adjacent LOADNULLS merging, and the implicit trailing return.
Unary minus is a NEG instruction, not compile-time folding. Floats are stored as
raw f32 bits. Native numeric lexing parses unsigned integer literals then
reinterprets their bits as i64; an out-of-range unsigned integer becomes zero.
Float lexing rounds a parsed f64 to f32, as native strtod/SQFloat conversion does.

`Vm::resume` adds its credit to existing operation debt, then charges before each
instruction dispatch. Suspension preserves registers and the next instruction.
The final return also consumes an operation. Each suspension consumes a charge;
credit one at zero debt therefore executes zero instructions. Register zero is
reserved for the unimplemented root environment and cannot be read or written by
scalar bytecode. Errors terminate a frame. Public bytecode operands are checked
at dispatch; unsupported opcodes fail explicitly.

Signed overflow and MIN/-1 remain unfinished compatibility work: native C++ does
not specify portable results. This increment returns `UnsupportedOverflow` rather
than claiming wrapping arithmetic is faithful. Unsupported syntax, the remaining
51 opcodes, locals/control flow, closures/functions, objects/reference lifetime/GC,
classes, generators, traps, standard library, packages, AI/GS APIs, scheduling and
saved callbacks are open obligations. Float NaN raw-bit witnesses are specific to
the tested native/Rust arm64 toolchain; no cross-platform payload guarantee is made.

## Reproduce native differential evidence

From the workspace root (output paths may be elsewhere):

```sh
scripts/compat/script-vm/build-native.sh .reference/OpenTTD /tmp/script-native
CARGO_TARGET_DIR=/tmp/script-rust cargo build -p ottd-script --example observe
scripts/compat/script-vm/differential.sh /tmp/script-native/observe /tmp/script-rust/debug/examples/observe /tmp/script-diff
scripts/compat/script-vm/controls.sh /tmp/script-diff
cargo test -p ottd-script
cargo clippy -p ottd-script --all-targets -- -D warnings
```

The native builder exports exact pinned sources rather than linking an existing
OpenTTD observer. The only host glue implements allocator and fatal/log diagnostics;
it makes no host API, resource-budget or game-integration compatibility claim.
Every script's original bytes, each observer stdout/stderr and exact invocations
are retained. Stdout compares bytecode/literals/stack/value and suspension/budget;
stderr diagnostics are separately retained, not advertised as matching prose.
Four controls deliberately change value, arithmetic opcode, suspension IP and
debt, and must all be rejected by the same diff comparator.
