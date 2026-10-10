# Squirrel scalar expressions and iteration foundation

This crate independently compiles a bounded subset of OpenTTD 15.3's bundled
Squirrel 2.2.5 and executes native register instructions. It has no C++ runtime
or native delegation dependency. It is not an AI/GS runtime and is not admitted
to game ticks.

Supported source includes scalar returns and expression statements, local
variables with optional initialization and grouped declarations, scalar local
assignment, blocks, if/else, while/for/do loops, scalar switch/case/default, break and continue. Values remain null,
bool, i64 decimal/octal/hex integers, and well-formed f32 decimal/scientific
literals. Expressions support parentheses, unary `- ! ~`, arithmetic `+ - * / %`,
comparisons `== != < <= > >=`, value-preserving short circuit `&& ||`,
bitwise `& | ^ << >> >>>`, comma and ternary expressions, scalar compound
assignments and prefix/postfix updates.
Identifiers are ASCII. Space, tab, CR and LF have exactly native's lexical meaning;
Line and block comments preserve the native distinction between newline tokens
and block-comment line bookkeeping. Semicolons and previous-token/newline
boundaries follow the bundled compiler.
Unsupported reserved words and compound tokens are rejected, including inside
unreachable source. Source size and recursive compilation are bounded.

Compiler targets distinguish named locals from temporary slots, preserving
assignment aliasing, shadowing and initializer lookup. For example,
`local a=1,b=2; return a+(a=b);` returns 4 as native does. The emitter preserves
native register high-water size, literal order, DLOAD/LOADNULLS/DMOVE merging,
ARITH destination redirection, literal EQ/NE fusion and optimization barriers.
Assignment validity follows native expression-state lifetimes, separately from the
current target register: `(a)=2` is rejected, while `a+a=2` assigns the temporary
expression result. Nested expressions restore their caller state.
It retains unreachable instructions and the implicit final return.

Float payloads are stored as raw f32 bits. Native same-type equality compares
raw payloads: positive and negative zero differ, while equal-payload NaNs compare
equal. Ordering deliberately follows native's raw-bit fast path and less-than
fallback; it is not Rust/IEEE partial ordering. Equality and relational operators
share native's left-associative precedence. Logical operators preserve the chosen
operand's type/value and skip the unevaluated operand's execution.

`Vm::resume` adds credit to retained operation debt, then charges before every
instruction dispatch. Taken branches use offsets relative to the next instruction.
Suspension preserves registers and IP. Every branch and scope cleanup costs one
operation; SCOPE_END uses native's conditional range and signed count, including
nested-loop nonpositive counts. Register zero remains the unimplemented root
environment and cannot be read or written by supported scalar instructions.
Public bytecode indices, taken branch targets and arithmetic are checked; invalid
bytecode returns typed errors. Runtime errors and returns terminate the frame.

Signed arithmetic overflow and MIN/-1 remain unfinished compatibility work:
Rust returns `UnsupportedOverflow` rather than claiming guessed wrapping behavior
is portable native semantics. Thirty-six opcode handlers remain unimplemented.
Foreach, strings, globals/objects, functions/closures,
reference lifetime/GC/classes/generators/traps, standard library/imports, host APIs,
AI/GS scheduling and Save/Load integration remain explicit later obligations.
Float NaN payload witnesses are specific to the tested native/Rust toolchain.

## Reproduce native evidence

```sh
scripts/compat/script-vm/build-native.sh .reference/OpenTTD /tmp/script-native
CARGO_TARGET_DIR=/tmp/script-rust cargo build -p ottd-script --example observe
scripts/compat/script-vm/differential.sh /tmp/script-native/observe /tmp/script-rust/debug/examples/observe /tmp/script-diff
cargo test -p ottd-script
cargo clippy -p ottd-script --all-targets -- -D warnings
OTTD_SCRIPT_VM_SOURCE="$PWD/.reference/OpenTTD" python3 scripts/check-contract.py --run script-vm
```

The native builder exports exact pinned sources into a private directory. Test
host glue only supplies allocation and fatal/log diagnostics; it makes no host API
or memory-limit compatibility claim. Default observer output includes instructions,
literals, stack size, return/error stage and suspension debt/IP. A separate
`--frames` native mode captures every scalar slot on suspension. Nine private
Rust VM tests compare these real captures without adding a production inspection
API, and CI reruns native captures against their pinned bytes. Additional budget
probes and branch-specific corruption controls are independently admitted.

Every original source byte and process argv/stdout/stderr/status is retained,
with actual binary identities, source snapshots and complete hashed archives.
Diagnostic prose is retained separately, not advertised as byte-identical.
The prior scalar/whitespace fixtures, test names and admission controls remain
required alongside the expanded stateful program corpus.

Shift operations follow the pinned C++20 build for all i64 patterns and counts
0 through 63, including negative left operands and discarded high bits. Other
counts return the distinct `UnsupportedShiftCount` error: native behavior there
is undefined, and portable compatibility remains unresolved. Prefix updates follow
native scalar expression targeting (`++2` is accepted); postfix local position is
separate from expression state. Compound and postfix stores preserve alias order.
For-loop increment instructions are extracted and re-emitted through the native
optimizer; this preserves native branch-layout quirks, including bounded suspension
of some ternary increments that might otherwise appear to terminate.

Switch selectors retain their original register reference; case EQ may overwrite
a named case operand. Fallthrough skips later case expressions. Switch owns break
targets, while continue targets the enclosing loop. Case scopes restore compiler
metadata; explicit break/continue preserve native conditional register cleanup.
