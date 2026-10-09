# Stage 4: gameplay parity implementation plan

Status: **In progress**. Branch: `stage/04-gameplay-parity`, based on the
maintainer merge of [PR #4](https://github.com/aron98/openttd-rust/pull/4),
`1eec4168398757644710eb1b0876cf8121eee922`. The approved roadmap authorizes this
stage. One stage PR collects verified atomic increments; only the maintainer's
merge authorizes stage 5.

Progress is collected in draft [PR #5](https://github.com/aron98/openttd-rust/pull/5).

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

The next parallel increments are vehicle sales, depot create/rotate commands
and NewGRF loading control. Loader control follows the original stage-major
order, with explicit standalone-batch scope; baseline-dependent queries remain
unsupported until their context is derived by the full Rust content loader.
Neither increment closes a broad stage requirement.

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
