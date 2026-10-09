# Compatibility foundation verification

## Saved-world commands and tick execution

On 2026-10-09, the stage 3 working tree passed the 18-case public replay matrix
against OpenTTD 15.3 at `14ec60f248547d4d062a1160f0fc26d742319888`.
The local run is `.artifacts/replays-Bx3roU/results/`; `provenance.json` retains
native, Rust and uninstrumented-original binary hashes, and each native
`invocation.txt` retains input/action and instrumentation hashes. These are local
observations from before the final integrated stage review and CI.

The clean implementation commit `6c93057fdbf43e6653320b1187fd271ddc31221b`
subsequently passed all eight baseline drivers and received independent
**APPROVE** with no blockers. [PR #4](https://github.com/aron98/openttd-rust/pull/4)
is ready for review. Both jobs in
[CI run 37921879534](https://github.com/aron98/openttd-rust/actions/runs/37921879534)
passed on the clean synthetic merge `24cdc5049b5b32df427c2d84d89e62b5cbf319cf`.
The [retained compatibility artifact](https://github.com/aron98/openttd-rust/actions/runs/37921879534/artifacts/11612926938)
(`compatibility-contract-37921879534-1`) contains the eight-driver report:
66 passing scenarios (63 implemented and three reference-only), with 18 future
scenarios explicitly not run, plus 201 passing Rust tests. The CI Rust job
separately records 13 passing Python tests.
Its replay evidence under `.artifacts/replays-LUhqTl/results/` covers all 18
cases, lifecycle checks and negative controls; all 2540 required paths are
nonempty. The original continuation again preserves road tile 3184 while
advancing ticks 8780 to 8796.

The native compiler cache recorded 617 hits out of 1050 cacheable calls
(58.76%), with 433 misses while building the new instrumentation. Local review
and baseline records are `.omo/evidence/stage03-final-gate.md` and
`.omo/evidence/stage03-baseline/report.json`; the downloaded CI report is
`.omo/evidence/stage03-ci-37921879534/.omo/evidence/contract-hz90jinf/report.json`.
These recorded results describe the implementation commit above. The latest PR
checks verify subsequent documentation-only commits. The maintainer merge is
still required; stage 4 has not started.

Reproduce after both native reference builds:

```sh
bash scripts/check-replays.sh
# Include the versioned capability and input/evidence checks:
python3 scripts/check-contract.py --run commands.world-ticks
```

The matrix covers command phases and costs; populated road construction;
ordinary ticks; month, quarter, year, leap day and tick-counter wrap; changing
grass; nonempty dormant-town history; low cash, pause gates, clear-limit refill
and nested naming. Three additional cases retain native large-integer behavior:
monthly interest saturates signed 64-bit Money before division; explicit loan
addition saturates before native validation/accounting; town history uses the
native signed 32-bit accumulator and final unsigned conversion. Independent
review found all three mismatches; dedicated Rust regressions and fresh original
execution now cover them.

Every case compares native and public-CLI receipts, complete saved descriptors
and raw map data, defined structural state, and deterministic runtime at the
same checkpoints. The Rust checkpoint saves are decoded again and compared to
the native saved trees. Only native host observers `interactive_random`,
`current_company` and supplemental numeric error metadata are outside the
deterministic observation projection; the originals are retained for audit.
No saved field is removed. Native `StateGameLoop` retains all timer registrations
and callback bodies; earlier isolated callback probes are not this evidence.

Fresh-process prefix/resume compares both engines with uninterrupted execution.
The prefix envelope retains the original eight actions at cursor position four,
including the four pending actions; its sibling save hash must match before
resume. The populated construction witness loads the Rust save in the
uninstrumented original, advances from tick 8780 to 8796 and preserves road tile
3184. Rust does not execute those populated gameplay ticks.

Eight controls reject wrong cost/order/clock/RNG/state, missing saves, missing
receipts and stale artifact directories. The contract names 2540 exact nonempty
paths across all cases, every checkpoint, resume and controls. PR-only CI retains
`.artifacts/replays-*/` without changing the native compiler cache. General
gameplay, content-dependent restoration, scripts, NewGRFs, UI and networking
remain outside the admitted replay domain; unsupported state or a later
unsupported event rolls back the request. See the
[stage 3 plan](stages/03-commands-tick-execution.md) and
[replay corpus](../fixtures/replay/README.md) for exact bounds.

## Typed snapshot milestone

Verified locally on macOS ARM64 on 2026-10-08 against the same pinned upstream
revision below. `cargo test --workspace --locked` passed 50 self-contained tests at initial integration;
the two external-oracle tests are explicitly run by `scripts/check-snapshots.sh`
instead of being counted as coverage from the ordinary test invocation.
Formatting and strict all-target/all-feature Clippy passed.

The differential run in `.artifacts/snapshots-RzmpVh/` passed all three fixtures
at 1 and 16 null-driver ticks. Each of the six directories retains the actual
C++ runtime `snapshot.json`, its paired `save/autosave/exit.sav`, two byte-identical
Rust snapshots, exact comparator results, typed-library parity results, and
primitive parity results. Tile, date, saved RNG and setting mutations all
returned nonzero with their exact paths and expected/actual values: 24 negative
controls total. No fields were filtered. The previous 12-case preservation matrix
also passed again in `.artifacts/compat-lFXq6R/`.

The new core tests replay actual upstream randomizer/scaling, calendar and map
vectors and check coordinate bounds, leap/century/max-year dates and checked date
arithmetic. The CLI additionally rejects duplicate JSON keys, inexact or
out-of-range numeric inputs, malformed JSON and byte-limit violations. Existing
rewrite no-clobber tests remain green. `snapshot` rejects historical versions;
the C++ engine performs the migrations in the differential fixture matrix.

This verifies typed map/clock/settings/randomness decoding and the deterministic
primitives. Simulation, game-object decoding, historical Rust migrations,
scripting, networking and UI remain unimplemented. CI now invokes both reference
matrices; the new CI configuration has only been validated locally at this stage.

## Earlier save-container milestone

Date: 2026-10-08. Platform: macOS ARM64. Rust: 1.96.0.
Upstream: OpenTTD 15.3 at `14ec60f248547d4d062a1160f0fc26d742319888`.
The GitHub releases API identified 15.3 as the latest stable release when pinned.

## Observed results

| Check | Result |
| --- | --- |
| Headless reference build, all four codecs | Built; `openttd -h` reports 15.3 |
| Strict Clippy, all workspace targets/features | Passed with `-D warnings` |
| `cargo nextest run --workspace --locked` | 23 passed, 0 skipped |
| `cargo test --workspace --doc --locked` | Passed; no doctests defined |
| CLI inspect/rewrite/help/error/no-clobber paths | Exercised through the executable |
| Shell syntax | Both shell scripts passed `bash -n` |
| Independent payload baselines | Original OTTN bytes or external XZ decompression matched Rust output exactly |
| Reference interoperability | Three fixtures x four formats passed (12 cases) |
| Negative oracle control | Failed load correctly rejected despite upstream exit code zero |
| Independent code review | CLEAR / APPROVE after all four findings were resolved |

The successful full matrix is retained locally in `.artifacts/compat-DkKvCM/`.
Each case includes the original independently decoded payload, Rust rewrites,
reference-engine logs, and exit saves. Successful comparisons excluded no chunks
or bytes. The generated fixture has a 64 x 64 map; the historical regression
fixtures contain more populated state. Their hashes and origins are in
`fixtures/README.md`.

## What the comparison proves

For each fixture, every compression rewrite preserves the original decoded
payload and loads in the original engine. For deterministic continuation, the
original engine first imports a fixture once. Both branches then start from that
shared native snapshot, run 64 null-driver ticks, and produce byte-identical
uncompressed saves after the Rust rewrite.

The engine executing both simulation branches is C++ OpenTTD. This demonstrates
that the Rust container rewrite preserves these inputs; it does not demonstrate
a Rust implementation of the simulation.

## Failures caught and fixed

- Initial tests ran against the empty codec/CLI and failed before implementation.
- A historical fixture exceeded the small synthetic-test byte budget; fixture
  tests now use the documented 256 MiB default without changing the limit tests.
- Independently importing older saves generated distinct random savegame IDs.
  Repeating the original-engine load reproduced the difference. Source:
  `src/saveload/afterload.cpp:3333` and `src/misc.cpp:87` at the pinned revision.
  One imported snapshot now provides identical starting state for both branches.
- Review exposed a false-positive oracle: failed loads fall back to the title
  world, save it, and exit zero. The negative case initially failed in
  `.artifacts/compat-bLJMmp/`. The runner now rejects upstream save/load error
  records and unexpected additional loads. That control passes in the final run.
- Review required an independent payload baseline; external `xz` and direct
  original-byte comparison now establish the expected bytes.
- XZ now uses bounded liblzma through xz2. The earlier candidate's dictionary cap
  did not also bound its index allocation. Both decoded bytes and codec memory
  have explicit limits; these are not a total process-memory ceiling.
- Randomized tests now reach chunk and compression parsing through valid headers,
  in addition to arbitrary whole-file input and structured round trips.

## Limits and environment notes

The initial save-container milestone had no Rust gameplay or object-field
interpretation. The later sections below record the typed snapshot, landscape,
and selected callback work; historical state migrations, UI, scripting, and
network implementation remain outside the implemented slices. It does not establish
compatibility for every old save or third-party content package. The full port
remains the product goal described in the README.

The GitHub Actions workflow was added but has not run on a remote runner. Local
execution validates macOS; Linux CI and Windows behavior are not claimed here.
The upstream build emitted a CMake CMP0177 policy warning and macOS deployment
target warnings for Homebrew libraries; it built and ran successfully. The Rust
checks passed without warnings. Editor LSP requests were intermittently cancelled;
the compiler, Clippy, tests, and real executable runs are the validation evidence.

These checks were performed locally before the initial repository publication.

## Clock and clear-landscape subsystem

Run `bash scripts/setup-snapshot-reference.sh` followed by
`bash scripts/check-simulation.sh`. The script starts a fresh native probe process,
then captures clock replay and CLI comparisons under `.artifacts/simulation-*`.
The native source and fixture provenance are documented in
[`reference/README.md`](../reference/README.md).

Coverage is 15 clock cases and 4 landscape cases with 22 checkpoints: 64x64 and
128x64 terrain, scheduling boundaries, recovery through 6144 ticks, pause and
u64 tick wrap. Every tile field, cached clock value, cursor, gameplay RNG and
ordered event record is compared. Repeated CLI output must be byte-identical.
Five deliberate mutations (tile, date, cursor, RNG, event) must exit unsuccessfully
and report the expected JSON path. Unit tests additionally reject unsupported
contexts, terrain, flood-prone void boundaries, malformed clocks and input limits.

This is isolated clock dispatch plus temperate clear-tile behavior, without timer
callback bodies, NewGRF callbacks, water simulation or game-object updates. The
positive-height invariant around void prevents the original void procedure from
leaving the supported terrain domain. These checks do not establish full-game
simulation parity or writing advanced state back to save files.


## Selected object callback bodies

Verified locally against the pinned OpenTTD 15.3 callbacks on 2026-10-08. Run
`bash scripts/setup-snapshot-reference.sh` and `bash scripts/check-callbacks.sh`.
The latter records each fresh native process, library replay, real CLI invocation,
comparison input/output, and diagnostic under `.artifacts/callbacks-*`.

There are 34 native cases: 12 vehicle, 8 house/company/station bookkeeping, and
14 original-economy industry cases. Each case is replayed through Rust and twice
through the public command (68 successful callback processes), with exact equality
of every modeled output field and byte-identical repeats. Regenerating the corpus
must reproduce `reference/callbacks.json` byte for byte.

Seven input controls reject malformed JSON, unknown callbacks, enabled vehicle
advice, closure-marked industries, NewGRF, smooth economy, and incomplete industry
history. Nine separate output controls mutate vehicle age/profit, group totals,
house age, company expenses, station status, industry history, RNG, and callback
phase. Each invokes the actual `ottd compare` command, requires failure with empty
stdout, and checks the exact diagnostic path and expected/actual integer values.
Unmodified comparison baselines must succeed. No comparator algorithm is duplicated
in these tests, and no modeled field is filtered from the positive comparisons.

The native probes select original registered timers by trigger and priority, or
invoke the original calendar vehicle scheduler. Industry phase is observed during
native callback dispatch, before day-count reset and maximum-year rewind. No
native callback body is copied into the oracle. Ordinary workspace tests replay
the committed native corpus; CI additionally regenerates it with the original
engine. Remote CI execution is not claimed by these local results.

These checks prove only the documented typed-state callback contracts. They do
not establish full-world ticks, object-pool save decoding, serialization of advanced
state, vehicle motion, daily operating callbacks, full town growth, cargo routing,
industry random policy/closure, UI, scripts, or network parity. The existing
landscape runner continues to report clock boundaries without dispatching these
object callback bodies automatically.

## Complete saved worlds and structural restoration

Stage 2 targets pinned version-362 saved state and content-independent
restoration. It does not claim content-dependent engine/spec/callback caches,
command execution or playable simulation. Those remain later-stage gates. The
[stage plan](stages/02-world-loading-saving.md) records review and integration
status separately from the executable capability declarations.

After both reference builds, run
`python3 scripts/check-contract.py --run world.complete-load-save` or
`bash scripts/check-worlds.sh`. The driver creates a fresh
`.artifacts/worlds-*/results/` tree, records native/Rust executable hashes and
checks all five retained [world fixtures](../fixtures/world/README.md). Saved
JSON comes from native memory during actual save traversal; structural JSON
comes from native objects after loading, before gameplay ticks. Comparison
retains every field, including script data and raw map planes.

One edit batch changes vehicles, companies, towns, industries, stations, orders,
cargo and infrastructure. Each family has its own before/after/path receipt,
and the complete expected native state must match after reload. Additional
cases change persistent storage, cargo payment state and a roadside map-plane
field, and exercise native saturating cargo-feeder totals. Modded-AI and
GameScript worlds also receive nonempty unrelated edits, with complete expected
state comparisons and explicit NGRF/AIPL/GSDT/PSAC preservation checks. Original
and Rust-edited content trees match exactly; native re-saves and script continuation
compare with unedited controls at matching load/save phases to retain native
Squirrel table ordering without normalization. The entire restored native world
must match its control plus the declared edit. Each native exit save is loaded again; its native
post-load structural state is compared with Rust decoding that identical file,
with the path and SHA-256 retained in a `resaved/input.json` receipt. Uninstrumented
original reload witnesses cover edited eight-family and storage/payment/map
saves, plus the edited modded and GameScript saves; their role is load acceptance,
not a fabricated JSON observation from an unmodified engine.

Eight controls require observable failure for a changed field, omitted field,
missing content, stale artifacts, absent AI restore marker, corrupted native
group cache, corrupted native cargo cache and corrupted cargo-payment binding.
The cache controls additionally require unchanged saved-state observations.
Native GameScript reload verifies a nonempty nested marker and goal/story/league
IDs. Native fixture commands and script execution establish source behavior;
Rust script execution is not claimed.

The contract requires concrete per-case artifacts and refuses missing/empty
evidence. Stage 2 introduced the seven-driver baseline; stage 3 adds the eighth
replay driver. PR CI retains the world and replay trees
alongside contract reports, including failures. A local passing run does not
substitute for the final independent review, CI, or maintainer merge.
