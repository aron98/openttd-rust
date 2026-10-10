# Squirrel scalar, string, root and scalar-array foundation

This crate independently compiles a bounded subset of OpenTTD 15.3's bundled
Squirrel 2.2.5 and executes native register instructions. It has no C++ runtime
or native delegation dependency. It is not an AI/GS runtime and is not admitted
to game ticks.

Supported source includes scalar returns and expression statements, local
variables with optional initialization and grouped declarations, scalar local
assignment, blocks, if/else, while/for/do loops, scalar switch/case/default, break and continue. Values include null,
bool, i64 decimal/octal/hex integers, raw f32 values, immutable byte strings and
identity-bearing arrays whose elements are structurally restricted to scalars.
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
nested-loop nonpositive counts. Register zero holds the actual configured root
identity; scalar instructions cannot treat it as a scalar operand.
Public bytecode indices, taken branch targets and arithmetic are checked; invalid
bytecode returns typed errors. Runtime errors and returns terminate the frame.

Signed arithmetic overflow and MIN/-1 remain unfinished compatibility work:
Rust returns `UnsupportedOverflow` rather than claiming guessed wrapping behavior
is portable native semantics. Twenty-eight opcode handlers remain unimplemented.
Foreach, general objects, functions/closures,
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
literal identity after construction. VM operands remain execution-checked. The convenience VM retains its Program borrow; persistent Runner frames instead
share the immutable ProgramData owner described below.

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
unsupported increment boundaries, separate from the lexer ctype policy. General
tables/objects, native closures, classes, require and host API registration remain
outside this slice. Compiler constants remain scalar; configured root slots
retain scalar values and same-realm scalar arrays through a bounded host interface.

Focused tests retain 37 original buffer observations: 36 closed-constant compiler
outcomes/bytecode/table snapshots are strict comparisons. The formerly unsupported
standalone unresolved-name read now compiles to the captured native GETK tuple
and fails at runtime with MissingIndex when the root and Realm are unpopulated. Populated
Realm reads, publication failures, ownership and temporary-update suspension
traces have separate tests. The full driver now declares 115 exact native constant
sessions, fresh public constant tests and three private stateful compile/lookup/execution projections.
Native refcounts and child release remain original observations paired with
separate Rust ownership tests; they are not claimed as full raw Rust session
parity. Four additional corruption controls exercise declaration publication,
counters, shared-state isolation and final native release. Admission still
requires newly executed complete debug and optimized profiles for these inputs.


## Configured roots and persistent runners

`Realm::empty_root` creates an independent plain root with byte-string keys and
scalar values or same-realm scalar arrays. RootEnvironment clones share slots; another empty root shares
only the Realm's interner/constants. `raw_get`, `new_slot` and `set_existing`
provide explicit own-slot host access, including empty/NUL-containing keys.
Newslot replaces existing entries; set requires an existing entry. Strings are
reinterned into the root's Realm. This configured empty root matches explicit
native table installation; it does not reproduce sq_open's base globals or the
OpenTTD host's delegated root initialization.

Unresolved identifiers and `::name` compile to native root GET/GETK and
LOADROOTTABLE, with `<-` and `=` stores using NEWSLOT/SET. Locals and inlined
constants keep compile-time precedence. Root reads see current runtime slots,
then default table delegates, then shared constants only under native's
receiver-equals-this fallback gate. SET never falls back to constants. A reached
default-delegate closure or late enum table returns UnsupportedRuntimeValue;
this is an explicit implementation boundary, not undefined lexer input.
Root compound/prefix/postfix updates, arbitrary this/receiver objects, root value
escape, dot members and generic table values remain unsupported. Array bracket
reads and plain assignments are supported; indexed compound/prefix/postfix updates
remain unsupported syntax.

`Runner` is persistent execution state, separate from shared root identity.
`compile`/`compile_bytes` use its Realm without clearing temporary state;
`start` borrows the runner for one frame and shares immutable ProgramData.
Replacing the root is idle-only and same-Realm-only. Two runners can share slots
while retaining separate temporaries. A cloned root can perform host writes
between frame resumes. The same existing Vm opcode loop serves borrowed runners
and `Vm::new`'s fresh owned convenience runner; there is no second interpreter.

Program clones share one literal pool. On first resume, the runner temporary owns
the real main-program data even when zero credit suspends before IP0 executes.
GET/ARITH/RETURN replace that owner in native order. A failed missing read may
leave the program owner alive after external handles drop and through subsequent
compilations. Terminal return/error releases active frame registers/program,
while the runner temporary persists. Caller-owned Returned values are separate
owners; the runner does not cache another result. Dropping an unfinished frame
abandons that host frame; this does not claim native cancellation/wakeup parity.

`last_failure` is typed Rust diagnostic history. Successful compile/run leaves
an older failure intact; precondition failures do not overwrite it. It is not a
native `_lasterror` string/value identity API. Diagnostic prose and its native
string-object ownership are excluded from exact projections and retained in raw
native captures. Unsupported runtime domains remain distinct from MissingIndex,
invalid bytecode, and the unchanged lexer policy.

The normal CI driver reruns all 90 declared native sessions on its fresh pinned
observer. Its selected production unit-test binary reads those fresh captures,
with seven additional public Runner tests. Four admission controls first accept
the original session, then reject only the declared changed stdout observation.
The pinned corpus manifest binds all 212 source/capture files and exact session
membership. Tests replay 69 supported native root sessions, three compile/call owner
sessions and three compile-root/run-root replacement sessions through production
handlers. They compare tuples, frames, IP/debt, root values, temporaries and
string-owner counts; only compile/runtime diagnostic prose is projected to typed
error stage. Fifteen additional native observations cover explicit unsupported
syntax/values and the supported shadowing of default delegates. These focused
observations do not replace newly executed full CI profiles and source-map closure
work required for admission of this increment. Full AI/GS execution, persistent
host API state, general heap/GC and game-tick integration remain open.


## Scalar arrays and host API migration

Array literals allocate on every execution, including repeated runs of one Program.
NEWARRAY reserves capacity with zero initialized elements; APPENDARRAY initializes
one element at a time, retaining the pinned literal-append fusion and opcode budget.
Bracket GET and plain SET preserve native evaluation, aliasing and temporary-owner
order. Empty arrays are truthy; `typeof` returns `array`; equality compares identity.
Arrays can be returned, stored in roots and held across programs, errors and suspension.
For example, fixed candidate scores can be scanned with `score[i]`, persistent
`pending <- [id, cost]` state can be updated by a later program, and shared local
aliases can update the same array without copying its contents.

The public `Value::Array(Array)` variant requires exhaustive callers to handle an
additional value type. `Realm::array(Vec<Value>)` constructs an opaque array;
`len`, `is_empty`, `get`, `set` and `same_identity` expose bounded host operations.
`RootEnvironment::new_slot` now returns `Result<(), VmError>`; callers must handle
foreign-array `RealmMismatch`. `set_existing` checks that affinity before lookup.
Both root stores preserve the old slot on rejection. Array strings are reinterned
into their owning Realm; arrays retain identity and cannot cross Realm boundaries.
`Program::from_parts` rejects array literals, so mutable data never enters the shared
compiler literal pool. No array storage or owner internals are publicly exported.

Private element storage has no container variant. Host construction, host SET,
VM APPENDARRAY and VM SET all reject a reached nested/self-array element with
`UnsupportedArrayElement`. An absent SET index returns MissingIndex first. This
structural restriction prevents cycles in the admitted ownership graph; it is not
an implementation of native GC. Native-valid nested arrays and self-cycles remain
explicit runtime boundaries, separate from the unchanged lexer policy. Relational
array ordering, pointer-bearing array stringification and reached default-delegate
closures are also explicit boundaries. Nonfinite or out-of-i64 f32 indices return
`UnsupportedIndexConversion`; defined finite indices truncate toward zero. Ordinary
missing indices and invalid operand types retain their distinct runtime errors.
Array methods/calls, foreach, resize/append host APIs, general tables and serialization
remain later dependencies.

Focused tests replay 78 retained native sessions through the production opcode loop.
They compare native tuples, literal pools, active registers, exact IP/debt, root
stores, array labels/contents, temporary ownership, weak-label death and string
owners. Native array reference counts, external handle-table bookkeeping, VM-root
bookkeeping and stack-top counts remain raw native observations; they are not
normalized to Rust counts. Only a witnessed no-op `collected 0` is omitted, and its
Rust replay asserts all watched arrays are already dead. Separate tests preserve
native-valid cycle/ordering/delegate observations while asserting typed Rust
boundaries. The old array-rejection body now has supported comparisons under all three feeds.
The normal driver declares 93 fresh native array sessions, retains their actual
argv/cwd/status/streams, and passes fresh captures to the production unit-test binary.
It requires 11 public array tests, 31 private crate tests, and four controls which
first admit an original session then reject a copied stdout mutation. The old root
array body is promoted across all three feeds: roots now have 75 supported sessions,
9 syntax boundaries and 6 runtime-value boundaries. Array accounting stays separate:
78 projections (six compile-only), 12 explicit boundary sessions and three protocol
refusals. Only the three declared array-stringification boundary captures have a
nonportable pointer spelling; admission checks native prefix/length and consistent
return/temp pointer identity, retaining raw text and comparing all surrounding state.
Malformed formats, missing records and changed surrounding state are rejected.
Both complete profiles remain required for this new CI source; prior focused proof
is not relabeled as fresh normal CI execution. Full AI/GS execution and all broad Stage 4 gates remain open.

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
including success or abnormal termination. The previously admitted driver has 10666 other observations as
strict comparisons; the array driver additionally requires the separately accounted session projections below. An unclassified typed rejection fails admission. Original
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
