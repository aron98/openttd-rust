# Stage 2: world loading and saving

Status: **In progress**. Branch: `stage/02-world-loading-saving`, based on the
maintainer merge of [PR #2](https://github.com/aron98/openttd-rust/pull/2),
`e35f0f7375d84124f8047adff404eb4273b7277b`. One stage PR will collect atomic
increments and await the maintainer's merge before stage 3 begins.

## Approved scope

The maintainer approved this boundary refinement on 2026-10-09: stage 2 decodes
and edits complete saved state, resolves object links and restores
content-independent derived state. Content-dependent runtime restoration moves
to the explicit stage 4 gate. The final compatibility goal remains unchanged.
A successfully loaded stage 2 world is not yet a gameplay-ready runtime.

The target remains pinned OpenTTD 15.3, commit
`14ec60f248547d4d062a1160f0fc26d742319888`, save version 362. Historical migration,
commands, ticking, pathfinding, script execution and NewGRF evaluation are outside
this stage. Mod identities, parameter values, load order, mappings, persistent
storage and serialized AI/GameScript state must survive unrelated edits.

Complete saved state means all native saved domains, not only map/settings or
the first populated pool: vehicles (including effect/disaster variants),
companies, towns, industries, stations/waypoints, orders/backups, cargo packets
and payments, infrastructure/map/depot/object/roadstop data, engines/renewals,
groups, subsidies, economy, linkgraphs/jobs/schedules, story/goals/league/signs,
content configuration and mappings, persistent storage, and script state.
Recursive table records must retain descriptors, nested structures, widths,
list order and sparse pool IDs. Any non-table or script-specific tail needs a
named format policy and explicit preservation checks; arbitrary opaque fallback
is not complete saved-state decoding.

## Native source and structural contract

`src/saveload/saveload.cpp` is authoritative for references: SLE_REF zero is null
and nonzero is pool index plus one. Ordinary ID fields have distinct invalid and
special sentinels. All pools are decoded before typed links resolve. Field type,
pool identity, duplicate IDs, dangling pointers, invalid variants, chain cycles
and multiple ownership must fail deterministically rather than hang or publish
a partially validated world. Historical source IDs that may legally outlive the
object must remain valid saved data.

The content-independent runtime contract covers vehicle consist first/previous
links, reverse shared-order links and orderlist membership; order counts and
timetable duration aggregates; cargo ownership/count/transit/feeder aggregates;
roadstop chain membership; group hierarchy; reverse persistent-storage and
cargo-payment bindings. Native authorities are `saveload/vehicle_sl.cpp`,
`order_cmd.cpp`, `saveload/cargopacket_sl.cpp`, `cargopacket.cpp`,
`saveload/station_sl.cpp`, `group_cmd.cpp`, and `saveload/afterload.cpp`.
The structural export contains:

- `vehicles`: `id`, `previous`, `first`, `previous_shared`.
- `order_lists`: `id`, `first_shared`, `vehicles`, `num_manual_orders`,
  `total_duration`, `timetable_duration`.
- `cargo_lists`: `owner`, `cargo_type`, `next_hop`, `packets`, `count`,
  `periods_in_transit`, `feeder_share`.
- `groups`: `id`, `children`; `road_stop_chains`: `station`, `kind`, `stops`.
- `storage_owners`: `id`, `owner`; `cargo_payments`: `id`, `vehicle`.

An `owner` identifies its typed pool and ID. Nullable links are JSON null;
saved-wire references remain native ID-plus-one values in the separate saved
tree. The implementations are documented in the
[world module](../../crates/ottd-save/src/world/README.md).

Content-dependent caches include vehicle capacity/length/speed/weight/rail-road
compatibility, town population/building/radius values, custom station flags and
catchment behavior, label remapping, infrastructure statistics influenced by
content, prices and airport noise. Native `AfterLoadGame` calls content loading
before these calculations; `Train::ConsistChanged`, `RoadVehUpdateCache`,
`Ship::UpdateCache` and `UpdateAircraftCache` may execute NewGRF callbacks.
These remain unimplemented runtime gates for stage 4, not default-filled values.
Rendering and viewport caches are outside the headless structural state contract.

## Ownership and internal milestones

1. Codec owner: `ottd-save` recursive table codec, sparse and variant framing,
   descriptor-preserving writes, native field coverage and script-tail policy.
   Own shared container/library exports. Tests prove malformed framing rejects
   and edits alter encoded values while unrelated records remain unchanged.
2. Runtime owner: `ottd-save/src/world*`, typed pool identity/reference validation,
   structural restoration and mutation validation. Own this plan and roadmap
   status. Consume the codec's agreed record API; do not duplicate wire parsers.
3. Oracle owner: independent native saved-state export, populated fixture corpus,
   conversion recipe, original load/re-save driver and structural export.
   Native output must come from native objects/handlers, not the Rust decoder.
4. Integration lead: CLI editing/export interface, compatibility metadata,
   integrated tests and independent review. Changes to shared interfaces are
   agreed before edits. No worker reverts another worker's changes.

The wire model is the authoritative saved state. Typed projections and derived
indexes must rebuild from it after edits; saving a stale untouched container
while editing a detached JSON view is invalid. Mutations are validated before
publishing output. Existing snapshot/simulation APIs remain supported.

## Acceptance checklist

- [ ] Every native v362 saved domain has a decoded representation and documented
      serialization/preservation policy, with no unexplained opaque chunks.
- [ ] Sparse pool identity, variants, references, null/special sentinels and
      ownership are validated; malformed controls fail without partial output.
- [ ] The named content-independent derived fields match native restored objects
      for populated worlds, including shared orders and nonempty cargo.
- [ ] Rust edits to each of vehicles, companies, towns, industries, stations,
      orders, cargo and infrastructure survive original load and are observed
      with exact expected values; unchanged fields remain equal.
- [ ] Original re-save of Rust-edited worlds reloads in Rust with equal saved
      state and the defined structural runtime state.
- [ ] Real mod/script fixtures preserve configuration, parameters, mapping,
      persistent storage, AI/GS serialized state and randomizers across edits.
- [ ] Native comparison negative controls detect changed values and missing
      fields; unexecuted native scenarios are never reported as passing.
- [ ] Formatting, strict Clippy, workspace and existing native regressions pass;
      fresh evidence records commands, input hashes, tested tree and results.
- [ ] Independent review resolves all blocking findings, required CI passes, and
      one stage PR is ready for maintainer review. No stage 3 work begins.

## Verification scenarios and artifacts

The retained [native corpus](../../fixtures/world/README.md) has five version-362
cases, each observed independently by the world driver:

| Case | Retained save | Nonempty coverage |
| --- | --- | --- |
| `populated` | `fixtures/world/populated-v362.sav` | Four transport families, towns, industries, stations, orders, cargo and infrastructure |
| `extended` | `fixtures/world/populated-extended-v362.sav` | Shared orders, parent/child groups, removed vehicle slot and nested WorldProbe AI data |
| `modded` | `fixtures/world/modded-v362.sav` | Authored GRF parameter123 and WorldProbe saved state; native engine speed observation |
| `storage-payment` | `fixtures/world/storage-payment-v362.sav` | Synthetic town/industry persistent storage, cargo-payment binding and native-built buoy waypoint |
| `game` | `fixtures/world/game-v362.sav` | Nested GameScript data, goal, story page/element and league table/entry |

Synthetic storage uses an inert authored GRF identity; it is not a callback
execution claim. Native script builders and restore markers prove serialized
state preservation, not Rust AI or GameScript execution.

For each native scenario retain input hashes and files, invocation log and exit
status, Rust saved/structural JSON, native saved/structural JSON, exact comparison
report, Rust edited save and native re-save. The binary observable is full
canonical equality or the precisely enumerated expected mutation delta. Byte
preservation alone is not the runtime comparison.

Malformed scenarios cover dangling references, duplicate pool IDs, wrong variant,
cyclic vehicle/shared/group/roadstop/renewal chains, multiple cargo/consist owners,
invalid map/pool association and out-of-range field writes. Legal deleted source
IDs and sentinel variants have separate positive cases.

Baseline required commands:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked
python3 scripts/check-contract.py --validate
python3 -m unittest discover -s scripts/tests -p 'test_*.py' -v
bash scripts/setup-reference.sh
REFERENCE_SOURCE="$PWD/.reference/OpenTTD" bash scripts/setup-snapshot-reference.sh
bash scripts/check-worlds.sh
# The contract entry point runs the same world driver and admits its evidence:
python3 scripts/check-contract.py --run world.complete-load-save
# Run all seven drivers for the integrated baseline:
python3 scripts/check-contract.py --run baseline
```

The world driver writes `.artifacts/worlds-*/results/`. Each named case has
`baseline/native/{world.json,derived.json}`, Rust equivalents and exact
`saved-compare`/`derived-compare` logs. `mutations/{vehicles,companies,towns,
industries,stations,orders,cargo,infrastructure}.json` records each edit's path
and before/after values; `mutations/change-compare/stdout.log` witnesses equality
of the complete expected native result. `storage-payment-map/` covers PSAC,
CAPY, a roadside map-plane edit and saturated feeder totals. `game-script-reload/`
contains the native nested-marker restoration evidence.

`negative-{mutation,missing-field,missing-content,stale,builder-marker,
group-children,cargo-cache,cargo-payment}/` retains the eight controls. Cache
controls also prove saved-state observations did not change. `unmodified/`
contains original-engine process receipts, loader logs and exit saves for
`eight-families`, `storage-payment-map`, `modded` and `game` Rust outputs; it does
not claim JSON instrumentation in the unmodified engine.

The [contract](../../compatibility/contract.json) requires 144 concrete nonempty
world artifacts. Its report records manifest/input hashes, tested commit and
working-tree state; PR CI retains the scoped world directories with the reports.
Local world-driver and contract-entry-point runs have passed. The integration
lead will reconcile the acceptance checklist after the final baseline,
independent review and required CI; the stage remains **In progress**.
