# Stage 4: gameplay parity implementation plan

Status: **In progress**. Branch: `stage/04-gameplay-parity`, based on the
maintainer merge of [PR #4](https://github.com/aron98/openttd-rust/pull/4),
`1eec4168398757644710eb1b0876cf8121eee922`. The approved roadmap authorizes this
stage. One stage PR collects verified atomic increments; only the maintainer's
merge authorizes stage 5.

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

- [ ] Record exact native source fields and Rust API before editing.
- [ ] Write failing comparisons for all vanilla climates and supported base
      specifications, including price settings, inflation and numeric bounds.
- [ ] Implement typed catalogs and price restoration with no fixture/runtime
      C++ dependency; retain source attribution for ported static tables.
- [ ] Export the corresponding original runtime values through a passive native
      observer, compare actual native output, and retain negative controls.
- [ ] Verify the atomic increment, review it independently and integrate it.

### B. Validated saved-object lifecycle

Extend `crates/ottd-save/src/world.rs` and `world/edit.rs` with explicit-ID
`InsertRecord`, `RemoveRecord` and `ReplaceRecord` edits for native pool tables.
Use `TableRecord` values and existing schema/limit/reference validation. Native
ID allocation policy belongs to runtime, not the save serializer. Add focused
tests in `crates/ottd-save/tests/world_runtime.rs` or a dedicated lifecycle test.

- [ ] Test linked cargo insertion and owner update in one transaction; compare
      rebuilt ownership/aggregates and serialized reload with expected state.
- [ ] Test unlink plus deletion and invalid intermediate-but-valid-final edits.
- [ ] Require duplicate insertion, missing remove/replace, invalid target pool,
      malformed schema, out-of-range IDs and dangling references to roll back
      the entire batch, including earlier unrelated valid edits.
- [ ] Implement the minimal explicit record operations. Preserve complete-world
      validation; do not bypass it or add runtime allocation to `ottd-save`.
- [ ] Reopen the resulting native save in original OpenTTD and compare complete
      saved/structural observations at matching lifecycle boundaries.
- [ ] Verify, independently review and integrate this prerequisite atomically.

### C. Native terrain geometry

Create `crates/ottd-core/src/terrain.rs` for typed slopes/corners/foundations and
`crates/ottd-sim/src/terrain.rs` for saved-map tile geometry. Authority:
`slope_type.h`, `slope_func.h`, `landscape.cpp` and `tile_map.cpp` in the pinned
native tree, including `GetPartialPixelZ`, `GetSlopeZInCorner`,
`GetSlopePixelZOnEdge`, `ApplyFoundationToSlope` and `GetTileSlopeZ`.

- [ ] Define checked native slope/foundation values and supported combinations.
- [ ] Test every valid slope across the 16-by-16 pixel grid, steep slopes,
      corners/edges, foundations and map borders against original functions.
- [ ] Implement geometry without modifying command/tick admission yet.
- [ ] Add a separate passive native observer and reproducible vector comparison;
      do not use a probe that disables ambient callbacks as whole-world evidence.
- [ ] Verify, independently review and integrate, then use the geometry in
      native construction commands with actual costs/limits/partial results.

## Second implementation wave

The first save-object increment is committed with native interop evidence. Its
independent review found that `ENGN` records must be contiguous from zero;
common world validation now rejects holes transactionally, and a fresh reviewer
verified the correction. Terrain and vanilla catalog increments have passed
isolated workspace checks and fresh exhaustive native comparisons; their
independent review remains a separate gate.

Proceed with these disjoint slices while reviews run. Fixes to a prerequisite
take precedence over dependent integration, and no milestone closes the stage.

### A. Road runtime restoration

Create `ottd-sim/src/runtime.rs`, `runtime/road_cache.rs`,
`runtime/saved_vehicle.rs`, `runtime/saved_engine.rs` and focused/runtime-native
tests. Add a passive `reference/runtime_road.hpp` observer and native runner.
The content owner also serializes shared module/observer registration.

- [ ] Introduce an owned `SimulationRuntime::restore_vanilla(World)` with private
      authoritative World, immutable content snapshot and only implemented
      typed caches; expose borrowed world/content/cache access and `into_world`.
      The initial constructor admits the declared ordinary road-vehicle domain;
      no placeholder caches or broad gameplay `advance` method are introduced.
- [ ] Restore actual native road/consist length, speed, power, cargo age period,
      mass, tractive effort, slope/axle resistance, drag and roadtype properties.
      Separate load-initialized trip/speed history from recomputable physics.
- [ ] Read saved ENGN state, cargo capacity, age/reliability and saved road paths
      through borrowed typed views. Do not rerun engine introduction RNG or
      overwrite saved fields with constructor defaults.
- [ ] Observe native after-load order and represent saved fixups explicitly.
      Compare empty/loaded buses and trucks, original/realistic acceleration,
      different engines, slope settings and sparse IDs. Reject unimplemented
      contexts before claiming restoration success.
- [ ] Verify complete saved-state/RNG preservation for no-fixup cases, exact
      runtime field equality, and wrong-cache/missing/stale negative controls.
      Invalidation and live ticking remain subsequent implementation work.

### B. Native ID allocation

Create `ottd-sim/src/runtime/pools.rs` with focused tests and a separate passive
native allocation observer. The transport owner owns allocation algorithms;
the runtime owner registers the module. Authority is `core/pool_type.hpp`,
`core/pool_func.hpp` and native `FreeUnitIDGenerator`.

- [ ] Reproduce logical pool allocation after sparse explicit-ID loading, first
      free selection, release/reuse, high-water behavior, reset and exhaustion.
      Keep native per-pool limits authoritative; do not substitute monotonic IDs
      or a different free-list order.
- [ ] Separately reproduce per-company/per-vehicle-type unit numbers, including
      their reserved values. These are not global vehicle pool indices.
- [ ] Compare operation traces against the actual native allocator templates
      and unit-number generator, including holes and boundary capacities.
- [ ] Keep allocation metadata transactional with its future World insertion.
      This increment must not expose unvalidated object creation or claim that
      logical allocation alone implements vehicle construction.
- [ ] Specify the saved-world journal/read-through transaction API before
      replacing whole-world cloning/serialization in hot gameplay paths.

### C. Terraform construction command

Create `ottd-sim/src/commands/terraform.rs` and command/native replay tests.
The world owner owns command enum/pipeline changes and replay command/return
instrumentation; coordinate shared library exports with the runtime owner.

- [ ] Add `TerraformLand { tile, slope: u8, dir_up }` with native AllTiles/Auto
      traits, raw low-four-bit corner mask semantics and native company/pause
      gates. Do not reject native successful zero/high-only masks as geometry.
- [ ] Preserve native requested-corner order, depth-first neighbor propagation,
      sorted two-pass surface checks, typed prices, limit/error/cash precedence,
      merged clearing/height changes and read-only tunnel obstruction checks.
- [ ] Extend receipts additively with typed test/exec/result tuple returns,
      preserving money and tile sentinels from the actual native command.
      Existing cost-only receipts remain unchanged. The native observer records
      actual return tuples instead of reconstructing them from input arguments.
- [ ] Compare basic/cascading/multi-mask changes, bounds, limits, cash, pause,
      tunnel obstruction and resumed execution on native prepared populated
      worlds. Validate all map bytes, receipts, money, limits, RNG and saves.
- [ ] Keep bridge-over-clear and other not-yet-ported tile procedures explicit.
      Follow with every remaining tile procedure and `LevelLand`'s distinct
      NoTest/partial-money semantics before closing construction parity.

Shared native headers/builds remain frozen during a running comparison. The
lead batches new hooks into scheduled builds, retains exact source/binary
provenance, and integrates each reviewed behavior atomically.

## Shared integration rules

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
