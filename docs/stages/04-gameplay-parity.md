# Stage 4: gameplay parity implementation plan

Status: **In progress**. Branch: `stage/04-gameplay-parity`, based on the
maintainer merge of [PR #4](https://github.com/aron98/openttd-rust/pull/4),
`1eec4168398757644710eb1b0876cf8121eee922`. The approved roadmap authorizes this
stage. One stage PR collects verified atomic increments; only the maintainer's
merge authorizes stage 5.

Progress is collected in draft [PR #5](https://github.com/aron98/openttd-rust/pull/5).

Latest reviewed content increment: `e032dd2` completes the bounded raw engine/spec
loading added in `d256ad1`. Script increment `ff0d636` adds configured scalar roots
and persistent execution runners. The latest admitted gameplay increment remains
`3d952b0`, for empty-road tile callbacks. Populated movement and cargo identity and
translation are in progress. All seven completion gates below remain open.
Earlier milestones and their bounded verification results are recorded below.

Private raw engine/spec loading preserves original construction/reset, scoped
vehicle-ID mappings and Road Action0 properties 08/09/0F/11. Extended local IDs
retain their u16 identity; substitute selection performs the native uint8
conversion before the table-count minimum. New non-original Road owners use the
native value-initialized running-cost class. Ten whole-state native/Rust cases
cover 48 API commands, six loader checkpoints and 280 native events, with eight
focused tests, 21 guards and 13 actual admission-corruption refusals. Verification
passes 614 workspace tests, strict Rust checks and 72 Python tests. Independent
review reproduces the original constructor failures, all 24 final engine tests,
and complete source/archive validation. Four adjacent copied-source rosters now
include the constructor inputs. Configured-world restoration, non-original Rail,
cargo/refit finalization and saved dynamic engine/EIDS restoration remain open.

Configured scalar roots retain shared slot identity while persistent runners keep
independent temporary ownership and failure history. Root reads and stores,
idle root replacement, compile-time constants and suspended execution follow the
pinned native semantics through the existing VM loop. Both complete debug and
optimized profiles pass 10,666 strict comparisons and 153 separately classified
lexer-policy rejections, 101 selected Rust tests, 90 root sessions and 115 constant
sessions. The root sessions distinguish 72 supported stateful projections from
18 typed implementation boundaries. Each profile validates all 93,531 indexed
artifacts and feeds fresh native captures into the selected Rust unit executable.
Root verification passes 629 workspace tests, strict Rust checks and 72 Python
tests. Independent review validates both complete packages, reruns 101 tests per
profile and 19 admission tests, and adds 48 native/Rust comparisons. General
objects, functions, host APIs, scheduling and full AI/GS remain open.

Empty-world ticks now visit admitted flat temperate normal roads and depots
through the existing landscape scheduler and saved-world transaction. Natural
visits change Barren roadside to Grass; depot and stable Grass state are
preserved. The registered driver checks eight original processes, six construction
commands, seven Rust replays and 100 process receipts. Root verification passes
568 workspace tests, strict checks, 66 Python tests and four fresh original/Rust
continuation pairs. Independent review verifies the complete archive and fresh
two-way save probes. Populated vehicle movement, roadworks, slopes, houses,
custom road types and broader road domains remain open.

Currency properties 0B-0F preserve rates, separator/position, exact symbol bytes
and euro years through activation and reload. The complete normal driver passes
14 API cases with 3,951 operations and 21 loader cases with 26 loads and 1,443
events. Root passes 562 workspace tests, strict checks and the existing property
0A driver. Independent review verifies both archives and reproduces 297 operations,
163,404 semantic controls and focused tests. Formatting, annual euro switching
and configured-content world restoration remain open.

Shared-order restoration composes live backup consumption with same-owner road
purchases in the default group. Native list insertion, full-capacity reuse,
property copying and shared timing resets publish atomically. Sixteen original/Rust
pairs cover 71 actions, 174 snapshots and 16 saves; the full 29-case owned-order
regression also passes. Independent review approves the increment after archive,
source and corruption checks. Nondefault groups remain outside this domain.

Owned script strings preserve length-delimited bytes, native escapes, comparisons,
concatenation, scalar formatting, instruction costs and suspended frame ownership.
Compilation realms share a weak string interner across programs. Root verification
passes 584 workspace tests, strict checks, 70 Python tests and 114 additional native
comparisons. The final complete debug profile passes 10,666 strict comparisons and
153 separately checked undefined-input rejections, with lifetime sessions, source
feeds and 23,071 raw-f32 formatting patterns. The earlier optimized profile tests
identical executed inputs; its actual source epoch remains explicit. Independent
review verifies the complete final package and reproduces 66 native comparisons,
1,030 float patterns, 18 private tests and four native lifetime sessions. Globals,
objects, functions, host APIs and full AI/GS remain open.

Compiler constants and closed enums now share a Realm-owned namespace. Declaration
publication follows native parser timing, including changes retained after later
compilation failures. Local shadowing, enum counters, replacement, interned names
and values, and temporary updates have original-derived checks. Both complete
debug and optimized profiles pass 10,666 strict comparisons and 153 separate
policy rejections, plus 115 native constant sessions and four actual validator
admission/rejection controls. Each profile runs 88 script tests and validates a
92,076-file evidence archive. Verification includes 592 workspace tests; final
strict workspace checks, 70 Python tests and 13 new admission tests pass.
Independent review additionally runs 96 differential comparisons, 115 native
sessions and both complete archive validators. Runtime root lookup, constant
postfix updates, minimum-integer declaration negation, general objects/functions
and full AI/GS remain outside this increment.

Tree clearing now preserves costs, clear limits, nearest-town selection and
rating changes through TerraformLand and LevelLand test/execute phases. Nested
test scopes share temporary ratings; successful execution publishes terrain,
town fields and company accounting through the saved-world transaction. The
new driver executes 117 original invocations, compares ordered events and
saved/runtime state, and checks observer independence, reload and split/resume.
Nine trace corruptions and eight unsupported-setting vectors are rejected.
Root verification passes 540 Rust tests, strict workspace checks, all 44 replay
scenarios, the context driver and the full 29-case owned-restoration regression.
Independent review reproduced 13 Rust selectors and nine Python tests and
verified both complete tree and restoration archives. Tree growth, water and
bridge construction, no-town and ground-vehicle/tree witnesses, and arbitrary
helper flags remain open; company 15 has state/result evidence only.

Currency property 0A includes owner defaults, deferred StringID mapping,
ordered writes, and reload/reset. Five direct API cases and 20 loader cases
cover 25 load passes; all six affected native drivers pass. Independent review
verified their complete archives and reproduced 23 native API operations,
14 Rust tests, 11 Python tests, and corruption rejection. Properties 0B-0F are
covered by the later increment above; formatting and annual euro conversion
remain separate work.

Owned/default-group order restoration composes purchase, backup consumption,
copied properties, allocation, payment/RNG, and runtime caches in one
transaction. The full driver matches 29 original/Rust pairs, 96 actions,
250 snapshots, and 34 saves, including full order-list capacity, sparse ID
reuse, implicit orders, signed timetable fields, client IDs, renewal rules,
and strict loaded continuation. Root verification passes 537 Rust tests,
strict workspace checks, 59 standard-library Python tests, 25 focused Python
tests, and complete purchase, sale, and order-lifecycle regressions.
Independent review verified all four archives and ran 27 Rust tests,
25 Python tests, three fresh native/Rust pairs, and 18 corruption rejections.
The later shared/default-group increment is described above. Nondefault-group
restoration, movement, and multiplayer dispatch remain required. The existing
fresh native duration exception is restricted
to proven new owned incarnations and requires Rust zero; loaded values stay
strict.

Latest branch-wide CI is pending. A preceding Linux run failed the script-VM
comparison; investigation of an earlier failure found platform-dependent
undefined byte-character classification in the original lexer. On 2026-10-10
the maintainer approved explicit Rust rejection when lexing reaches that
undefined input domain, while preserving defined behavior and Unicode in
ordinary strings and comments. `2c74d24` implements typed rejection and separate
CI accounting; its [Linux run](https://github.com/aron98/openttd-rust/actions/runs/38041414571)
passes both jobs. `f55927b` preserves defined octal continuation through the
original char-narrowed predicate. Later string support extends the policy to
reached hexadecimal escape classifiers while retaining defined Unicode payloads.

Depot removal matches 34 original-engine cases, 117 actions, 302 snapshots and
69 save checkpoints. Shared orders, infrastructure, pool reuse, costs and
rollback are checked together. Legal above-ground occupancy witnesses remain
an explicit gap; the live-backup sale sequence below covers same-tile removal.

Review caught supplementary Unicode being accepted inside script comments.
The correction matches native lazy decoding, malformed-byte rejection,
encoded-surrogate acceptance and NUL termination through `compile_bytes`.
Both debug and optimized profiles pass 7,642 native comparisons, 59 tests and
nine native frame witnesses. A fresh reviewer approved the correction and
original supported comments/switch after another 1,132 comparisons. The
integrated workspace passes 467 tests, strict Clippy and formatting; 58 Python
tests and the complete order/depot comparison drivers also pass. Strings,
globals, objects, host APIs and full AI/GameScript behavior remain required.

`de84670` adds sale of stopped vanilla single-part road vehicles with owned or
shared orders, with backups disabled and an empty backup pool. Order detachment,
last-owner list removal, refunds and runtime allocation publish through one
validated transaction. Twelve original-engine pairs, 28 actions and 14 strict
save checkpoints pass. The integrated workspace passes 472 tests; all six
affected native drivers pass. Independent review approved the increment after
six fresh native/Rust pairs and complete source/archive checks. `4a90c5d` adds
its native observer prerequisites, and `5f8357d` corrects the language evidence
result inventory. The observer also passed all 44 existing baseline replays
using the preceding reviewed Rust executable.

`3c28479` admits sale with live order backups in ordinary single player while
keeping backup creation disabled. Backup cleanup observes the original shared
chain and publishes with order detachment, vehicle removal and refunds in one
transaction. Ten original/Rust pairs cover 49 actions, 118 snapshots and 15
saves, including backup, sale and depot removal without reloading. Four
renewal-state regressions caught and corrected an accidental guard relaxation
before integration. Independent review approved the corrected commit after
seven fresh pairs, eight controls and five observer refusal tests.

`851b504` adds NewGRF session strings with generic Action 4 names and the admitted
Action 13 translation domain. One private table preserves allocation order,
first-definition defaults, language selection and exact fallback bytes. Original
comparisons cover ten API cases and 90 loader cases, with 102,315 comparator
rejections and seven invocation guards. The integrated workspace passes 502
tests. Language, context, 248-case loader control, order-state and live-backup
sale regressions also pass. Baseline/specification owners, arbitrary formatted
errors, broader finalization, and content-dependent world restoration remain
open; this does not admit arbitrary modded worlds.

Backup-enabled sale now composes backup creation, candidate-state cleanup and
order detachment before one validated publication. The admitted domain remains
stopped vanilla single-part road vehicles in ordinary single player. Local
command client zero becomes server client one; the backup primitive keeps a
literal user zero. Same-slot replacement and the full 255-slot pool are checked
against the original operation order. Existing backup-disabled behavior and
renewal-state guards remain enforced.

Sixteen original/Rust pairs cover 73 actions, 178 snapshots and 21 saves. These
include 25 command receipts, 20 executed commands and 21 successful results;
refused commands are distinct from successful differential comparisons. Fourteen
native invocation refusals, eleven focused Rust tests, five semantic mutations
and executable/source admission controls pass. The integrated workspace passes
512 tests, with 59 standard-library Python tests and 42 sale-evidence tests.
Restore, purchases with live backups, vehicle movement and multiplayer command
dispatch remain separate requirements.

Linux CI exposed a separate script compatibility boundary: the original lexer
passes decoded characters above 255 to byte character-classification functions.
The same surrogate-encoded input compiles on the observed Linux build and fails
on macOS. The historical strict comparison remains failed. The approved policy
requires explicit rejection at the reached classifier, including identifier,
number and string-escape lookahead, rather than whole-input Unicode rejection.
Local macOS results do not establish portable behavior for that domain. The
explicit policy now passes the Linux run linked above; historical divergent
observations remain preserved.

Private work continues on mutable engine specifications, scoped engine mappings
and road properties 08/09/0F/11, with live CI integration still pending. Complete
native snapshots cover constructor/reset state, allocation and partial failures;
an additional unknown-property/following-record boundary is under investigation.
Configured-world content restoration remains blocked until its full dependencies
are implemented.

Original-only movement observations now include a vehicle built and started by
real commands, a loaded 37-tick road crossing, and 74 uninterrupted ticks compared
with a 37-tick save/load split. Final original saves are byte-identical. Saved
state and captured noninteractive runtime agree; each process retains its own
observed, unsaved interactive RNG inputs, whose differences remain recorded.
This evidence supports the first Rust movement implementation; it is not yet
Rust/native movement parity. The broad stage gates below are unchanged.

For agentic workers: use the subagent-driven development workflow, with explicit
ownership, native-source evidence, independent review and tests before integration.

**Goal:** execute the pinned OpenTTD gameplay inventory in Rust, including
content-dependent restoration, existing NewGRFs and Squirrel scripts, with native
state equality and two-way persisted continuation across representative worlds.

**Architecture:** keep `ottd-save::world::World` authoritative for saved state.
Restore content and gameplay runtime in `ottd-sim`; the save crate must not
depend on simulation. Host filesystem, content acquisition, clocks and transport
remain outside deterministic simulation. Native C++ is an independent test oracle,
never a runtime implementation behind a Rust wrapper.

**Target:** OpenTTD 15.3, upstream
`14ec60f248547d4d062a1160f0fc26d742319888`, save version 362; Rust workspace,
native original-engine checkpoints, and Python/CMake verification drivers.

## Scope and completion gates

This is the full stage 4 boundary agreed in the roadmap. An initial vehicle,
construction command or content catalog is an internal milestone. Passing it
does not complete the stage or authorize omitting a gameplay family.

| Contract requirement | Required behavior and evidence |
| --- | --- |
| `world.content-dependent-restoration` | Vanilla and modded engine/specification, vehicle physics/capacity, house population, industry, station/catchment, infrastructure, price and airport-noise runtime matches original after load and after changes. Save/reload reconstructs the same runtime and preserves continuation. |
| `gameplay.construction-terrain-water` | Construction/removal and ownership for roads/trams, rail/signals, stations/airports/docks, bridges/tunnels, depots, objects, terrain and water. Costs, errors, partial-success semantics, limits and effects match. Climate/snow/desert, animation and flooding execute in original phase order. |
| `gameplay.vehicles-pathfinding` | Rail, road, ships and aircraft, including supported native subtypes and consists: movement, routing/reservations, orders, loading/refits, depot/service/reliability/breakdowns/aging and replacement. Native saved state, runtime and RNG agree through relevant boundaries. |
| `gameplay.cargo-stations` | Generation, destinations, routing/linkgraphs, transfers/payments, packets, station facilities/ratings/catchment and periodic effects, with deterministic job publication and save/reload. |
| `gameplay.towns-industries-economy` | Real town/house growth and authorities; all industry economy modes, production/building/closure; prices/inflation, subsidies, company finance, bankruptcy and mergers. Observe map/pool creation and destruction, money and RNG. |
| `content.newgrf` | Unmodified content identity/dependencies, parameters, load order, feature variables/callbacks/sprites and persistent state. Test more than one feature and interacting content; saved-byte preservation alone is insufficient. |
| `scripts.ai-gamescript` | Compatible OpenTTD Squirrel semantics, versioned APIs, events, instruction budgets, scheduling, commands and save/load of existing scripts. Exercise both AI and GameScript; the original's modified VM is the authority. |

UI and playable network transport remain their later stages. Simulation-side
command order, persistent-storage transitions and host-independent state must
remain suitable for those stages. Existing admitted-domain rejection stays in
place until the corresponding behavior is implemented and verified; it is not
evidence that the missing behavior is complete.

## Parallel ownership and dependency order

Three workers can run alongside the integration lead in this environment. Use
all three for independent slices. Rotate an available slot into independent
review after implementation freezes; an author cannot approve their own change.

- **Content/runtime owner:** specifications, content resolution, NewGRF and
  script dependencies; new `ottd-sim/src/content*` modules and content tests.
- **Transport owner:** vehicle/order/cargo/station behavior; initially the shared
  saved-object lifecycle prerequisite in `ottd-save/src/world*` and its tests.
- **World owner:** geometry, construction, terrain/water, town/industry/economy
  behavior; initially new `ottd-core/src/terrain.rs` and its simulation adapter.
- **Integration lead:** shared interfaces, stage plan, serialized atomic commits,
  root QA, native build scheduling, independent reviewers and the single stage PR.
  Shared-file integration is assigned explicitly to one worker at a time.

Run the following dependency waves. Each substantive slice is committed with
its direct regression/native evidence before dependent work expands it.

1. **Independent foundations:** content/prices/catalogs; transactional pool
   records; terrain geometry. None depends on another's unfinished product API.
2. **Runtime restoration:** engine/vehicle and station/town/infrastructure
   projections, exact native pool allocation, cache invalidation and runtime
   ownership. Content-free restoration remains in the save crate.
3. **First complete transport cycle:** construction/depot/vehicle/order commands,
   road vehicle movement, town cargo, station loading/delivery/payment, service
   and persisted continuation. Expand the whole-world loop only with matching
   native phase effects, including ambient terrain/economy work.
4. **Concurrent gameplay families:** rail/signals/reservations, ships/water,
   aircraft/airports; world growth/industries/company lifecycle. Content loading
   and callback execution progress alongside the systems that consume them.
5. **Content and scripts integration:** complete NewGRF feature callbacks and
   persistent storage, compatible Squirrel execution/API scheduling, modded
   vehicle/house/industry/station scenarios, and content-aware after-load parity.
6. **Whole-stage closure:** cross-family interactions, full command inventory,
   runtime restoration, negative controls, saved continuation, compatibility
   declarations, independent reviews and PR CI. No required family is deferred.

## First implementation wave

### A. Source-derived vanilla content and price catalog

Create typed modules under `crates/ottd-sim/src/content/`, with a public module
and focused tests. Derive specifications from pinned native tables and native
initialization, retaining field widths, landscape mappings, cargo labels,
engine types and native price arithmetic. Explicitly distinguish base specs from
mutable saved engine state and NewGRF-resolved properties. Do not manufacture
modded properties from vanilla defaults.

- [x] Record exact native source fields and Rust API before editing.
- [x] Write failing comparisons for all vanilla climates and supported base
      specifications, including price settings, inflation and numeric bounds.
- [x] Implement typed catalogs and price restoration with no fixture/runtime
      C++ dependency; retain source attribution for ported static tables.
- [x] Export the corresponding original runtime values through a passive native
      observer, compare actual native output, and retain negative controls.
- [x] Verify the atomic increment, review it independently and integrate it.

### B. Validated saved-object lifecycle

Extend `crates/ottd-save/src/world.rs` and `world/edit.rs` with explicit-ID
`InsertRecord`, `RemoveRecord` and `ReplaceRecord` edits for native pool tables.
Use `TableRecord` values and existing schema/limit/reference validation. Native
ID allocation policy belongs to runtime, not the save serializer. Add focused
tests in `crates/ottd-save/tests/world_runtime.rs` or a dedicated lifecycle test.

- [x] Test linked cargo insertion and owner update in one transaction; compare
      rebuilt ownership/aggregates and serialized reload with expected state.
- [x] Test unlink plus deletion and invalid intermediate-but-valid-final edits.
- [x] Require duplicate insertion, missing remove/replace, invalid target pool,
      malformed schema, out-of-range IDs and dangling references to roll back
      the entire batch, including earlier unrelated valid edits.
- [x] Implement the minimal explicit record operations. Preserve complete-world
      validation; do not bypass it or add runtime allocation to `ottd-save`.
- [x] Reopen the resulting native save in original OpenTTD and compare complete
      saved/structural observations at matching lifecycle boundaries.
- [x] Verify, independently review and integrate this prerequisite atomically.

### C. Native terrain geometry

Create `crates/ottd-core/src/terrain.rs` for typed slopes/corners/foundations and
`crates/ottd-sim/src/terrain.rs` for saved-map tile geometry. Authority:
`slope_type.h`, `slope_func.h`, `landscape.cpp` and `tile_map.cpp` in the pinned
native tree, including `GetPartialPixelZ`, `GetSlopeZInCorner`,
`GetSlopePixelZOnEdge`, `ApplyFoundationToSlope` and `GetTileSlopeZ`.

- [x] Define checked native slope/foundation values and supported combinations.
- [x] Test every valid slope across the 16-by-16 pixel grid, steep slopes,
      corners/edges, foundations and map borders against original functions.
- [x] Implement geometry without modifying command/tick admission yet.
- [x] Add a separate passive native observer and reproducible vector comparison;
      do not use a probe that disables ambient callbacks as whole-world evidence.
- [x] Verify, independently review and integrate the geometry prerequisite.
- [ ] Use the geometry in native construction commands with actual
      costs, limits and partial results.

## Second implementation wave

The first save-object increment is committed with native interop evidence. Its
independent review found that `ENGN` records must be contiguous from zero;
common world validation now rejects holes transactionally, and a fresh reviewer
verified the correction. Terrain and vanilla catalog increments have passed
isolated workspace checks and fresh exhaustive native comparisons. Independent
review approved the catalog, terrain and observers at `8c4ef60`, with no blockers.
That tree passed 220 Rust tests, strict workspace Clippy and formatting. Native
comparisons covered 52 catalog configurations and exhaustive terrain vectors in
both map-edge modes; deliberately altered observations were rejected. These are
prerequisite milestones, not evidence of complete gameplay execution.

Proceed with these disjoint slices while reviews run. Fixes to a prerequisite
take precedence over dependent integration, and no milestone closes the stage.

### A. Road runtime restoration

Create `ottd-sim/src/runtime.rs`, `runtime/road_cache.rs`,
`runtime/saved_vehicle.rs`, `runtime/saved_engine.rs` and focused/runtime-native
tests. Add a passive `reference/runtime_road.hpp` observer and native runner.
The content owner also serializes shared module/observer registration.

- [x] Introduce an owned `SimulationRuntime::restore_vanilla(World)` with private
      authoritative World, immutable content snapshot and only implemented
      typed caches; expose borrowed world/content/cache access and `into_world`.
      The initial constructor admits the declared ordinary road-vehicle domain;
      no placeholder caches or broad gameplay `advance` method are introduced.
- [x] Restore actual native road/consist length, speed, power, cargo age period,
      mass, tractive effort, slope/axle resistance, drag and roadtype properties.
      Separate load-initialized trip/speed history from recomputable physics.
- [x] Read saved ENGN state, cargo capacity, age/reliability and saved road paths
      through borrowed typed views. Do not rerun engine introduction RNG or
      overwrite saved fields with constructor defaults.
- [x] Observe native after-load order and represent saved fixups explicitly.
      Compare empty/loaded buses and trucks, original/realistic acceleration,
      different engines, slope settings and sparse IDs. Reject unimplemented
      contexts before claiming restoration success.
- [x] Verify complete saved-state/RNG preservation for no-fixup cases, exact
      runtime field equality, and wrong-cache/missing/stale negative controls.
      Invalidation and live ticking remain subsequent implementation work.

Road restoration and its coverage correction passed independent review at
`1f6945b`: 24 configurations, 870 native rows, sparse IDs through 128, all eight
admitted road tile categories and 32 precise unsupported-shape checks. Saved
JSON, encoded bytes and RNG remain unchanged. Live cache invalidation and
vehicle movement are separate outstanding gates.

### B. Native ID allocation

Create `ottd-sim/src/runtime/pools.rs` with focused tests and a separate passive
native allocation observer. The transport owner owns allocation algorithms;
the runtime owner registers the module. Authority is `core/pool_type.hpp`,
`core/pool_func.hpp` and native `FreeUnitIDGenerator`.

- [x] Reproduce logical pool allocation after sparse explicit-ID loading, first
      free selection, release/reuse, high-water behavior, reset and exhaustion.
      Keep native per-pool limits authoritative; do not substitute monotonic IDs
      or a different free-list order.
- [x] Separately reproduce per-company/per-vehicle-type unit numbers, including
      their reserved values. These are not global vehicle pool indices.
- [x] Compare operation traces against the actual native allocator templates
      and unit-number generator, including holes and boundary capacities.
- [ ] Keep allocation metadata transactional with its future World insertion.
      This increment must not expose unvalidated object creation or claim that
      logical allocation alone implements vehicle construction.
- [x] Specify the saved-world journal/read-through transaction API before
      replacing whole-world cloning/serialization in hot gameplay paths.

### C. Terraform construction command

Create `ottd-sim/src/commands/terraform.rs` and command/native replay tests.
The world owner owns command enum/pipeline changes and replay command/return
instrumentation; coordinate shared library exports with the runtime owner.

- [x] Add `TerraformLand { tile, slope: u8, dir_up }` with native AllTiles/Auto
      traits, raw low-four-bit corner mask semantics and native company/pause
      gates. Do not reject native successful zero/high-only masks as geometry.
- [x] Preserve native requested-corner order, depth-first neighbor propagation,
      sorted two-pass surface checks, typed prices, limit/error/cash precedence,
      merged clearing/height changes and read-only tunnel obstruction checks.
- [x] Extend receipts additively with typed test/exec/result tuple returns,
      preserving money and tile sentinels from the actual native command.
      Existing cost-only receipts remain unchanged. The native observer records
      actual return tuples instead of reconstructing them from input arguments.
- [x] Compare basic/cascading/multi-mask changes, bounds, limits, cash, pause,
      tunnel obstruction and resumed execution on native prepared populated
      worlds. Validate all map bytes, receipts, money, limits, RNG and saves.
- [ ] Keep bridge-over-clear and other not-yet-ported tile procedures explicit.
      Follow with every remaining tile procedure and `LevelLand`'s distinct
      NoTest/partial-money semantics before closing construction parity.

Shared native headers/builds remain frozen during a running comparison. The
lead batches new hooks into scheduled builds, retains exact source/binary
provenance, and integrates each reviewed behavior atomically.

## Saved-world transaction prerequisite

The allocation primitive in `2e35e16` passed independent review.
Its isolated tree passed 224 Rust tests, strict workspace Clippy and formatting;
fresh original pool traces matched 1,177 operations plus 17 unit-number
operations, and a changed cursor was rejected. The save transaction work below
is independent of that frozen primitive.

The transport/save owner implements a sparse transaction overlay in `ottd-save`,
with minimal private table/snapshot adapters. The runtime and construction
owners retain their simulation files. No simulation dependency enters the save
crate. The current clone/encode/decode edit path remains a differential test
oracle until the new boundary passes its equivalence checks.

- [x] Provide `World::transaction`, staged `apply` and candidate reads, fallible
      `prepare`, then infallible `commit`. Retain each touched record or tile once.
      Failed staging poisons the transaction. The public transaction leaves World
      unchanged until commit, including when safe code uses `mem::forget`.
- [x] Candidate reads observe prior writes without exposing a partially valid
      World or stale derived indexes. Prepared reads expose validated candidate
      projections so simulation caches can be computed before publication.
- [x] If prepare temporarily installs candidate values to reuse validators, keep
      that swap/restore guard private and unexposed. No user callback executes
      while candidate values are installed; all error/unwind paths restore the
      original before returning a prepared overlay or failure.
- [x] Share exact wire validation and budget accounting with table encoding;
      reuse native limits, semantic, reference, ownership, chain, script and
      snapshot checks. Validate final coupled state, preserving temporarily
      invalid intermediate relationships and dense ENGN loading requirements.
- [x] Avoid whole-world cloning and whole-save serialization during mutation.
      A complete validation and structural reconstruction once per batch is an
      intermediate implementation, not a claim of incremental validation.
- [x] Compare all edit variants with the existing implementation, including
      repeated writes, insert/remove/reinsert, nested lists, map-object links,
      malformed data, failure after valid changes, and uncommitted drop.
      Reopen coupled output in the original and compare full saved/derived state.
- [ ] Coordinate tentative allocator, RNG and simulation cache values under one
      runtime borrow: all fallible work precedes saved-state commit and the
      remaining publication operations are infallible. Failed cache computation
      must still roll back the prepared saved transaction.
- [ ] Measure the batch boundary on realistic maps/pools before admitting broad
      populated ticks. Preserve all compatibility checks when optimizing it.

The saved transaction prerequisite in `61acfe3` passed independent review:
108 focused save tests, independent lifecycle/map/unwind probes and a fresh
original-engine reload with complete saved/derived comparisons. Integration
checks passed 248 workspace tests and all 29 replay cases. The prepared-discard
test was subsequently clarified in `725711a`; it makes no cache-failure claim.
Road purchase now exercises cache/allocator publication; other mutation families
and populated-tick performance remain open.

## Shared integration rules

### NewGRF file scanning milestone

The v1/v2 framing and bounded FILESCAN implementation `1f6945b` passed
independent review, 64 fresh native scanner cases and 96 additional native
boundary probes. It retains compressed physical spans, sprite variants,
native checksum extent, raw Action8 metadata and skip-handler behavior.
Comparator controls were strengthened in `7abb45d` and all 64 cases rerun.

Action14 metadata now has ordered recursive nodes, localized text, parameter
masks/defaults/ranges, version and palette rules, and original text-code
translation. Its fresh-config native matrix covers 1,720 scans, 2,446 direct
translations and 17,200 compatibility decisions, with 11,456 altered-observation
controls. The integrated Rust tree passed 280 tests and strict workspace Clippy;
the actual metadata CI dispatcher and its archived control evidence passed.
Independent review approved the increment with no findings, including another
1,792 mixed text comparisons and the unchanged 64-case native scanner matrix.
This increment assumes fresh language/string registries; later loading must
supply the populated registry context.
Loading passes, properties, spritegroups, callbacks,
modded runtime restoration and Squirrel execution remain required. Parsing
files does not activate content or satisfy the broad `content.newgrf` gate.

### LevelLand execution extension

The clear/void TerraformLand increment `ac73117` passed independent review,
235 workspace tests and all 29 replay cases. Its propagation and surface helpers
are separated before expansion. The next command uses the saved transaction
overlay; it must preserve the original's distinct NoTest semantics.

- [x] Add a private terrain reader over either committed World or CandidateView,
      with immutable map dimensions captured from the validated starting world.
      Reuse borrowed records and typed prices; do not introduce a general World
      trait or expose partially valid mutable state.
- [x] Preserve native rectangle and diagonal iterator order, target-height rules
      and raw mode validation. Test the outer command even with NoTest enabled.
- [x] Estimates repeatedly query unchanged terrain and preserve the native
      local-limit decrement/order, including limit-one zero-cost behavior.
- [x] Execution observes each successfully staged prefix. Failed native steps
      add no edits; partial-money completion retains its prefix and reports the
      next full step cost, not the cash deficit. Test, execution and final costs
      and tuples may differ.
- [x] Stage prefix and final accounting in one transaction, with one terminal
      prepare/commit. Never clone, encode or validate the complete world for
      each nested Terraform call. Rust scope/invariant failures remain explicit
      and roll back the candidate instead of impersonating native partial success.
- [x] Compare ordinary/diagonal selections, overlap, limits, cash, errors and
      save/resume against actual native dispatch, with altered extra-money,
      prefix and order controls. Initial tile scope matches current clear/void
      Terraform; remaining tile procedures are subsequent required increments.

The LevelLand increment `b0720d5` passed 262 isolated workspace tests and all
44 replay cases, including 15 new LevelLand cases, controls, save/resume and
original continuation. All 5,689 required replay evidence paths were present.
Independent review approved the increment with no findings, including additional
native boundary probes; this does not close construction parity.

### Road service and native CI admission

The service-interval command in `ceca244` preserves original owner/error order,
percent/calendar/wallclock bounds, company defaults and flag updates. Its ten
native scenarios cover 88 command actions, complete saved/derived comparisons,
original reloads and split/resume, with five comparator controls and 475 process
receipts. The owned runtime admits this cache-independent mutation. Vehicle
construction, movement and actual servicing remain separate requirements.

Dedicated foundation and service drivers now run their native matrices in
pull-request CI. Independent review passed service semantics but found that a
renamed terrain test could silently execute zero tests. `9edc1bf` requires exact
test execution; the reviewer's actual zero-test dispatcher probe now fails and
the normal driver passes. `4e2fcfe` gives the service CLI a typed argument
boundary; the exact scoped type check now reports no warnings. Fresh independent
review approved both corrections with no findings, reproducing the real negative
probe, normal foundation run and all 475 service process receipts.

### Owned-runtime road purchases

The purchase increment `5d4a78a` constructs every saved field explicitly for
single-part vanilla road vehicles. Allocation, unit numbers and RNG remain
tentative until saved-state validation and creation-cache computation succeed.
The original acceleration model's creation caches differ from after-load caches;
native observations compare them before saving. The obsolete `cargo_paid_for`
serializer global is retained separately, including a nonzero value, instead of
being reset when a new vehicle is constructed.

The native matrix covers 16 cases, 345 actions and all 88 vanilla road engines,
including four climates, both acceleration models, sloped depots, failures,
limits, cash and save/resume. It compares 194 successful executions and rejects
27 altered observations. The dedicated CI driver verifies all 1,011 process
receipts, 6,844 retained evidence files, and the identity of the two actual Rust
test executables across 19 invocations. Integration passed 295 Rust tests,
22 Python tests, strict Clippy/formatting and all 44 existing native replay cases.
Review found that the affordability check incorrectly rejected a zero-cost
purchase when company cash was negative. `86ad8c3` restores the original
positive-cost guard and adds the exact regression to the native CI matrix.
Fresh independent review approved the correction after reproducing the exact
native mismatch, the expanded CI matrix and rejection of a zero-test run.
The follow-up `dc9146e` separates immutable purchase planning and cohesive test
modules. A fresh review verified unchanged function bodies, assertions and
native test selectors; the full expanded purchase matrix still passes.
The original `BaseConsist::round_trip_time` has no initializer. Linux emitted
48 for a fresh stopped vehicle; native allocator poisoning emitted the exact
0xAAAAAAAA allocation pattern. This is an explicit fresh-initialization
exception: Rust initializes new vehicles to zero, while every loaded value is
preserved. The purchase comparator admits only that signed-int32 native field
for IDs proved by matching successful execution receipts, absent at initial
load, and still stopped in depot with no orders. Every admitted path, native
value and creation ordinal is recorded without changing raw worlds or saves.
Initial/resumed vehicles, other fields, receipts and all reloads remain strict;
every Rust checkpoint is also compared strictly against its decoded save.
Sixteen additional real comparator rejection controls enforce the boundary.
This purchase-only lifetime model rejects ticks and commands other than build;
future sale/depot matrices must explicitly track reset, removal and ID reuse.
Refits, articulated/modded vehicles, order
backup restoration, sales and movement remain subsequent required work.

### Depot runtime restoration

`5c94483` adds original loader/depot observation hooks, and `bac7236` restores
depot allocation metadata and all 63 road/tram infrastructure counters per
company. The native matrix covers sparse depot IDs, all counter slots, road
surfaces, ownership and both ends of bridges and tunnels across 428 vectors.
The counter-only road-type fixtures restore the original registry and tiles;
complete saved-state comparisons confirm that the probes leave no changes.
They do not claim activated NewGRF road-type compatibility.

Native comparison exposed a tunnel endpoint bug: height means the minimum of
the four clamped tile corners. A shared corner reader now supplies both that
query and the existing slope query. Integration passed 302 Rust tests, 22 Python
tests, strict checks and the expanded foundation CI driver. Independent review
approved both commits after another native 428-vector run and source audit.
The observation hooks also preserve all 44 existing replay cases and metadata
comparisons. Depot create/rotate/remove commands remain subsequent work.

### Configured NewGRF loading control

`261b4f7` implements the original stage-major LABELSCAN, INIT, RESERVE and
ACTIVATION order for configured standalone Action6/7/8/9/D/10 control programs.
The executor preserves ordered config and dynamic-file identities, filename
aliases, labels, conditional jumps, parameters and persistent byte overrides.
Source, record visits, labels, override work and emitted trace storage have
explicit cumulative limits; host limits never become native disabled results.

The corrected original-engine matrix matches 248 case identities, 9,711 events,
7,727 configured record decisions and 1,549 final configurations. Its actual
comparator rejects 4,766 altered observations. The dedicated CI driver checks
complete case membership, actual test-executable identities, source hashes,
5,353 retained raw/archive files and real stale/reused/zero-test/subset guards.
Independent review found that maximum-file rejection must preserve labels on
an earlier file with the same filename. `a73999c` fixes that native divergence;
a fresh reviewer approved the correction after executing the counterexample,
focused tests and complete evidence audit.

The native oracle executes its original baseline files. Rust independently
scans their actual source identities and rejects queries that need their
unexecuted runtime state; it does not fabricate baseline status or properties.
This control profile cannot yet publish a gameplay content catalog. Actual
baseline sprite/group/language handlers, properties and finalization, callback
execution and content-dependent restoration remain required next work.

### Road sales and depot creation

`cd7f904` implements sale of supported fresh, stopped, single-part vanilla road
vehicles, with transactional saved removal, pool/unit reuse and survivor cache
preservation. All/default group and engine counts derive from authoritative
records. Order backups, shared orders, cargo destruction, aged/group profit
and renewal lifecycle remain explicit unsupported dependencies.

The native sale matrix covers 22 cases, 571 commands and 370 executions across
88 engines, with 26 corruption controls and 4,542 archived matrix files. Its
native uninitialized-duration exception is bound to successful build/sale
incarnations and occupied IDs. Reuse cannot inherit eligibility from a loaded
vehicle; Rust fresh values must be zero. Loaded fields, other state, receipts,
self-decode and original reload comparisons remain strict.

`c92868e` creates and rotates vanilla road depots on supported clear terrain.
It preserves native validation and cost precedence, including nested failure
expense/foundation cost, naming holes and maximum-ground-corner occupancy.
Tile and depot insertion validate together before allocation and infrastructure
publication. `9df6b13` adds the complete native CI driver: 178 scenarios,
676 commands, 201 executions, seven corruption controls, original reload and
split/resume, with 49,998 archived files and 182 actual test bindings.

The combined product passed 333 workspace tests, strict Clippy/formatting and
20 focused Python tests. Independent review approved exact `9df6b13`, audited
complete source/executable/raw evidence, revalidated all saved/live/receipt
comparisons and ran five fresh native cases. Linux PR CI for this revision is
separate from that local approval. Depot removal, broader clearing/occupancy,
orders/destructors and vehicle movement remain required.

### Loading context and occupancy witnesses

`ff4226c` adds test-only original loader-context and road-ground observers;
production Rust never delegates gameplay to them. `ebf0e49` supplies typed
saved clocks/settings to the existing private loader executor, including
network normalization/restoration, globals, patch flags and special targets.
Public control-profile admission remains bounded until actual baseline,
specification, resource allocation, language/error/safety and callback handlers
are complete.

The integrated private context matches 66 original cases, 67,828 events,
67,300 decisions and 121 configurations. Its comparators reject 1,980 altered
context fields and 66 altered traces; 18 actual host-refusal controls pass.
Host date-domain admission errors remain distinct from native GRF disabling.
`8b1132c` separately checks four original road occupancy cutoffs on valid flat
and sloped sources, with complete saved/live/reload and hash-order restoration
and an actual corrupted-result rejection. This does not admit arbitrary
above-ground saved vehicles or mixed vehicle families.

The combined source passed 346 workspace tests, strict checks and direct
66/18/4 native executions. The new original observer also passed all 44 existing
replays and the 428-vector depot regression. `aab990e` adds complete CI admission,
retaining 1,586 context and 535 occupancy evidence files with 80 and eight native
invocation bindings. Independent review approved this increment after repeating
the context matrix, guards and actual occupancy restoration/control probes.

### Scalar script execution and road-slope prerequisites

`c822d7e` and `9eb9cc5` introduce the initial scalar Squirrel compiler, register VM
and original-VM CI comparisons. Review found that Rust accepted form-feed
whitespace rejected by native Squirrel; `733d5a9` fixes that boundary and retains
actual control-byte fixtures. Fresh review approved the correction and initial
scalar domain. Debug and optimization-level-3 dispatchers each pass 414 native
comparisons and 16 tests; each archive retains 5,012 evidence files, 73 source
inputs and 1,523 pristine native source blobs. The remaining 51 opcodes, objects,
GC, host APIs and full AI/GameScript scheduling and persistence remain required.

`1757e19` and `b724296` add original CheckRoadSlope observations and a private Rust
primitive. All 155,648 combinations of natural slope, requested/existing/other
road bits and build-on-slopes setting match, including requested-piece mutation
on failure. Five signed price probes and four actual corrupted-output rejections
pass. Complete saved/live state and temporary settings/prices restore. The
integrated workspace passes 368 tests and the new observer passes all 44 existing
replay scenarios, controls and save/resume checks. This helper remains test-only
until a real road command consumes it; it does not implement BuildRoad or removal.

`4bf5419` moves the unchanged 5,689 replay evidence requirements into a SHA-pinned
literal asset. The effective drivers and scenario declarations are unchanged;
the root manifest's 1 MiB cap remains, with separate bounded external reads.
`a69c7c1` adds the road-slope CI driver and complete 223-file archive admission.
Independent review approved these road-slope and manifest increments, including
fresh native comparisons and complete evidence admission.

The Linux PR run for `9df6b13` passed both jobs, including loader, sale and depot
milestones, with 1,046 of 1,050 native compiler calls served from cache (99.62%).
The subsequent PR run for `733d5a9` passed its Rust job and all 18 baseline
drivers, but evidence upload failed because test-selector colons appeared in
artifact paths. `e6f3f63` encodes evidence path components while preserving
original process arguments, checks complete archive paths, and prints bounded
failure diagnostics while retaining full stderr. `3765b4f` also fixes standalone
Python test discovery; `b38418e` updates the exact context freshness evidence
membership after adding road and safety observers. Independent review approved
these corrections. The `e6f3f63` run successfully uploaded evidence but exposed a
stale VM source pin for the changed shared helper. The scalar integration
refreshed that pin. The subsequent `4a556df` run passed both Rust and the complete
interoperability baseline, including evidence upload, with 1,044 of 1,050 native
compiler calls served from cache (99.43%). CI runs on pull requests only.

### Static NewGRF safety and continuing dependencies

`acc6a5d`, `170bf4c`, `98bfaac` and `cbcf859` add original static-safety
observations, native lazy FILESCAN behavior, Rust safety scanning and complete
CI evidence. The original FillGRFDetails path supplies 893 cases and 1,867
decisions; 13,794 deliberate comparison alterations and ten host-refusal controls
are rejected. The final integrated proof also reruns context, occupancy and road
slope against the same immutable native build. All 385 workspace tests and
strict checks pass, and independent review approved exact `e6f3f63`.

This scanner does not activate gameplay content or execute arbitrary callbacks.
Current parallel work covers real language-pack inputs and ordered language
maps, further Squirrel syntax and runtime dependencies, and command-level depot
removal using the reviewed shared-order/live-backup primitives below. Each
remains subject to original-engine proof and independent review.
Diagnostics/inhibition, GRM, actual baseline/spec execution,
the remaining VM/object/host API behavior and gameplay consumers are still
required. None of these increments closes a broad stage requirement.

### Scalar locals and structured control flow

`a26db01` and its review correction `5e2dcf4` add local declarations and
assignment, comparisons, short-circuit expressions, blocks, if/else, while,
break and continue. The compiler preserves native target-register aliasing,
optimization barriers and expression-state lifetime; the VM preserves branch
offsets, private register cleanup and operation debt across suspension. Review
caught both incorrectly accepted parenthesized assignments and incorrectly
rejected native-valid expression targets. The correction ports the original
expression-state rules and keeps the destination register separate.

Fresh debug and optimization-level-3 runs each pass 3,058 original/Rust
comparisons, 30 public tests, four native-backed private-frame tests and eleven
corruption controls. The full workspace passes 403 tests and strict checks.
Fresh independent review approved `5e2dcf4` after another 1,332 differential
comparisons and complete source, executable and archive verification. Each
profile retains 26,827 evidence files. This is 21 of 62 opcode handlers;
remaining operators, objects, GC, calls, host APIs and full AI/GameScript
scheduling and persistence remain required. Remote CI for this increment is
separate from these local acceptance results.

### Canonical shared orders and live backups

`16c3fa2`, `822d48a` and `d4403e9` add native order observations and actual
network-received save capture, canonical ORDL/VEHS/BKOR runtime operations, and
complete CI admission. Host role is explicit and immutable at load. Runtime
single-player/client saves omit transient backups; offline World serialization
remains lossless. Loaded client pool slots and original object indices remain
distinct. Native assertions on deleting nonzero loaded slots are recorded as
boundaries, with Rust rejecting before any state publication.

The original/Rust driver passes 19 pairs, 89 actions, 216 snapshots, 39 save
checkpoints and 40 vanilla airport geometry rows. Thirteen semantic corruptions,
nine capture mutations, zero/subset execution and native freshness controls are
rejected; three native assertion cases are excluded from successful parity.
All 421 workspace tests, both prior scalar profiles, context comparisons and
44 replay/control/resume cases pass. Independent review approved `d4403e9`
after a fresh shared-order challenge and full source/archive verification.
BackupRestore, ordered sale, command-level depot removal, movement and custom
airports remain required. The native client fixture is not mixed multiplayer
compatibility.

### Scalar updates and iteration

`0e9362b` adds bitwise operations and shifts, comma and ternary expressions,
arithmetic compound assignment, prefix/postfix updates, and for/do loops.
It preserves native expression-state and destination aliasing, update store
order, increment re-emission quirks and suspension debt. Defined shifts accept
all integer bit patterns with counts 0 through 63; undefined counts have a
distinct typed boundary rather than a successful compatibility claim.

Debug and optimization-level-3 native profiles each pass 5,638 comparisons:
618 fixtures under nine schedules plus 76 targeted budgets. All previous
fixtures remain; 280 distinct additions cover the new behavior. Each profile
passes 37 public and seven private tests, seven native frame witnesses and
16 corruption controls. Root's additional 408 comparisons and the reviewer's
118 comparisons match. Ten actual implementation mutants and 19 admission
attacks are rejected. The integrated workspace passes 431 tests, and the order
driver passes after its shared source bindings are updated. Independent review
approved exact `0e9362b`. An early dependency-free unittest now detects stale
live VM source pins, with a real stale-helper red/green witness.

This reaches 26 of 62 handlers. Comments/switch/foreach, strings and objects,
call frames and closures, GC, host APIs, scheduling, persistence and documented
native-undefined arithmetic domains remain open. None of these increments
closes a broad stage 4 gate.

Tree/water additions must account for nested test-only town ratings, nearest-town
tie rules, Auto clear-limit bypass, ownership, vehicle geometry and neighboring
water bits. Add those coupled effects separately after the LevelLand boundary;
do not append them as unverified branches to the existing clear-only procedure.

World operations must preserve native identity, iteration/allocation order and
ownership. Changes spanning records and tiles validate as one transaction.
Performance improvements must retain the same safety and wire constraints;
cloning or decoding the full world per object per tick is not a viable final
runtime architecture.

Runtime restoration and execution use explicit content inputs, deterministic
RNG and cache ownership. Saved fields and disposable caches must not become
competing authoritative models. Cache invalidation after a command is tested
against the same fields restored after a save/reload.

Follow native `StateGameLoop`: persistent-storage enter, animation, calendar
timers and vehicle calendar scheduler, economy timers, tick timers, tile loop,
vehicle ticks, landscape ticks, persistent-storage leave, AI, GameScript and
landscaping limits. Paused GameScript behavior is included. Do not skip an
unimplemented callback or alter RNG order to make a scenario match.

Each gameplay family adds commands, native observations and meaningful world
fixtures together. Compare complete saved fields/raw map, defined runtime,
receipts, object lifecycles and RNG at matching native boundaries. Native
fixture preparation and after-load are distinct from replay execution. Expand
the runtime observation schema deliberately; never normalize away differences.

The integration lead schedules shared reference setup/builds so workers cannot
overwrite one another's instrumented source or binary. New observer files can
be authored in parallel; common patches/setup scripts have one owner per wave.

## Stage acceptance and handoff

- [ ] Every seven-family requirement above has original-versus-Rust scenarios,
      including relevant vanilla/modded state and save/reload during execution.
- [ ] Content-dependent after-load state and invalidation after commands match.
- [ ] All vehicle classes and construction/terrain/water families are covered.
- [ ] Cargo/station, town/industry/economy/company lifecycle effects agree.
- [ ] Existing NewGRF and AI/GameScript execution passes the declared baseline.
- [ ] Cross-family command/tick ordering, RNG and persisted continuation agree.
- [ ] Native evidence freshness and wrong-state/order/cost/RNG controls fail.
- [ ] Existing stage 1-3 regressions remain green; CI runs only for PRs and
      preserves native compiler caching and auditable artifacts.
- [ ] Independent reviews and required local/PR checks pass; one stage PR is
      ready for maintainer review. Stage 5 has not started.

Per-increment verification includes meaningful red/green regressions and native
comparisons. Integration gates remain `cargo fmt --all -- --check`, strict
workspace/all-target/all-feature locked Clippy, workspace tests, Python contract
tests/validation and the complete versioned baseline. External ignored tests
must run through their actual drivers. New claims and exact required evidence
are added to `compatibility/contract.json` only when executable evidence exists.
