# Squirrel scalar, string and realm foundation

This crate independently compiles a bounded subset of OpenTTD 15.3's bundled
Squirrel 2.2.5 and executes native register instructions. It has no C++ runtime
or native delegation dependency. It is not an AI/GS runtime and is not admitted
to game ticks.

Supported source includes scalar returns and expression statements, local
variables with optional initialization and grouped declarations, scalar local
assignment, blocks, if/else, while/for/do loops, scalar switch/case/default, break and continue. Values remain null,
bool, i64 decimal/octal/hex integers, raw f32 values and immutable byte strings.
Normal, verbatim and character literals follow native escapes and encoded-byte
length. `typeof` returns interned type-name strings. Expressions support parentheses, unary `- ! ~`, arithmetic `+ - * / %`,
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
is portable native semantics. Thirty-five opcode handlers remain unimplemented.
Foreach, globals/objects, functions/closures,
cyclic graph ownership/GC/classes/generators/traps, standard library/imports, host APIs,
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
`--frames` native mode captures every scalar slot on suspension. Twelve private
Rust VM frame tests compare these real captures without adding a production inspection
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

`compile_bytes` follows the pinned compilebuffer callback: it decodes only the
current lookahead, rejects malformed UTF-8 and codepoints above U+FFFF, accepts
native encoded surrogates, and stops at NUL without inspecting
the suffix. `compile(&str)` delegates to this same path. Diagnostics retain
original byte offsets. This is not OpenTTD's separate BOM/default file loader.


`Realm` shares a weak string interner across compilations and runtime operations.
Cloning a `ByteString` retains immutable length-delimited bytes, including embedded
NUL; it does not promise UTF-8. `Realm::compile` / `compile_bytes` retain that
realm in the compiled Program. Standalone compile functions use a fresh realm.
`Value` and `Execution` are owned, non-Copy types. A returned string remains valid
when its VM, Program and Realm wrappers drop. Weak intern keys disappear on final
release; this is string ownership, not a collector for future cyclic objects.

`Program::from_parts` replaces public struct construction, checks stack size and
reinterns supplied string literals into a fresh realm. Read-only accessors preserve
literal identity after construction. VM operands remain execution-checked. The VM
continues borrowing its Program; no new host or call-frame abstraction is provided.

String concatenation converts supported scalars using native formatting; f32
shortest digits use pinned ryu 1.0.23 under its BSL-1.0 option, with bundled fmt's
notation thresholds, exponent spelling, NaN/infinity signs and negative zero.
String ordering compares unsigned bytes, equality uses live intern identity, and
empty strings remain truthy. Local ++/-- use native addition/concatenation, including
aliasing and temporary targets. These operations add no extra opcode charges.

Return/error releases active registers. Native `temp_reg` retains the last ARITH
result and is overwritten by RETURN; the Rust VM now preserves that owner until
replacement/drop. The old scalar observer could not expose this lifetime: its
failed-update assertion now also observes the exact failed store before terminal
unwind, retaining the previous store-order guarantee.

Native --realm/--parallel/--terminal sessions bind lifetime counts and suspended
owner interactions to private Rust tests. The --format batch compares 23,071 raw
f32 patterns. Separate --feed unsigned/UTF-8 observations demonstrate original
LoadFile callback differences without claiming a file/BOM loader implementation.
`Realm` also shares a private compiler constant table. `const` accepts integer,
float and byte-string literals (or numeric unary minus); closed `enum` members
use those values or an independent implicit counter starting at zero. Explicit
member values do not advance that counter. Declarations own interned names and
values; local variables shadow them, and programs inline the value observed at
compilation. Replacement cannot rewrite an older program's literal.

Publication is not a transaction around successful compilation. Const publishes
after scalar lookahead and its internal terminator check; enum publishes after
the closing-brace token is obtained but before its following Lex call. A malformed
byte adjacent to that brace can fail while the token itself is being decoded;
separating whitespace can let publication happen before the later error. Prior
publications survive later compilation failures. Same-line continuation after a
const semicolon can fail the outer terminator check after publication.

Defined compound/prefix updates operate on loaded temporaries and leave table
bindings unchanged. Direct `=` assignment rejects after parsing its RHS. Constant
postfix operations and minimum-native-integer declaration negation are explicitly
unsupported increment boundaries, separate from the lexer ctype policy. Runtime
root lookup, arbitrary tables/objects, newslot syntax, native closures, classes,
require and host API registration remain outside this slice. No public Value
variant or root-object API was added. The Vm still borrows its Program; tests do
not pretend a native independently retained closure is that Rust borrow.

Focused tests retain 37 original buffer observations: 36 closed-constant compiler
outcomes/bytecode/table snapshots are strict comparisons, and the standalone
unresolved-name read is an explicitly tested runtime-root boundary. Populated
Realm reads, publication failures, ownership and temporary-update suspension
traces have separate tests. The full driver now declares 115 exact native constant
sessions, fresh public constant tests and three private stateful compile/lookup/execution projections.
Native refcounts and child release remain original observations paired with
separate Rust ownership tests; they are not claimed as full raw Rust session
parity. Four additional corruption controls exercise declaration publication,
counters, shared-state isolation and final native release. Admission still
requires newly executed complete debug and optimized profiles for these inputs.


## Defined lexer input policy

OpenTTD 15.3 (`14ec60f248547d4d062a1160f0fc26d742319888`)
passes decoded codepoints to byte ctype in `sqlexer.cpp`: Lex 251/255/261,
ReadID 443, and ReadNumber 376/383/388/397/408. Arguments 0..255 are
within the defined byte domain; U+00E9 is not an undefined argument merely
because its source encoding uses multiple bytes. The standalone observer uses
the initial C locale, where identifiers are ASCII.

When a reached classifier would receive 256..65535, Rust rejects with
`UndefinedNativeCharacter { codepoint, context }` at the original byte offset.
This is the explicit input policy for undefined native behavior, not native
compile-error parity. Identifier terminators, numeric prefixes and numeric
terminating/exponent lookahead are included. Decoding remains lazy: comment
payload, NUL suffixes and input beyond an earlier syntax error are not eagerly
classified. Supplementary and malformed encodings retain native decoder failure.

CI retains the eight original exact Token-policy bodies and adds nine exact
HexEscape-policy bodies (first digit, after one digit and after four digits, for
U+0100, U+20AC and the native encoded U+D800 codepoint). These seventeen bodies
have an `undefined_native_input` stage: 153 credit cases independently
require the typed Rust rejection and retain raw original argv/status/stdout/stderr,
including success or abnormal termination. The other 10666 observations are
strict comparisons. An unclassified typed rejection fails admission. Original
Linux U+D800 success and macOS rejection remain divergent observations; neither
is normalized, rewritten or claimed as portable semantics.

ReadString 306/310 guards only reached hex-escape classification, including the
lookahead after four digits because `isxdigit` precedes the length bound. Ordinary
BMP Unicode and native encoded-codepoint string/comment payload remain supported.
A byte-domain U+00E9/U+00FF terminator and a fifth ASCII hex digit followed by
Unicode remain defined comparisons. Malformed/supplementary decoder failures,
NUL EOF and invalid-escape errors keep their lazy ordering. Fixtures/categories
make these boundaries explicit. Admission requires complete debug and optimized
evidence bound to the candidate’s consumed script inputs.


Octal continuation is a distinct defined path: `scisodigit(char)` narrows to an
eight-bit character and compares `0`..`7`; it is not a byte ctype call. Once an
ASCII second digit has selected octal, a decoded BMP value whose low byte is
0x30..0x37 continues that loop. Native APPEND_CHAR retains the full UTF-8 encoding;
ParseInteger then refuses trailing nonnumeric bytes and its value_or(0) yields
integer zero. Encoded native surrogate values behave the same way. Rust preserves
this path, including later malformed-byte errors, invalid ASCII octal digits and
NUL termination. The subsequent `isdigit` lookahead remains guarded. The initial
zero-prefix `toupper` is still an undefined argument for decoded values above255;
an observed native acceptance there does not make it a defined octal input.
