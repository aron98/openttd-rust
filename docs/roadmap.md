# Compatibility roadmap

## Product goal

Reimplement OpenTTD faithfully in Rust, initially targeting OpenTTD 15.3 at the
revision in [`upstream.toml`](../upstream.toml). Original and Rust clients must
be able to play together in the same session, whether the server runs the
original implementation or Rust. Saves and existing compatible add-ons must
work in both implementations without conversion by content authors.

Use one deterministic Rust engine for the desktop client, browser client and
headless dedicated server. The preferred client direction is a shared Rust
renderer with native and WebAssembly builds; `wgpu` is a candidate, not a
committed dependency. Browser connections to unmodified original servers need
a compatible network gateway. Tauri is not required by this architecture.

Compatibility is version-specific. The first target is 15.3, not arbitrary
original releases or source-modifying patchpacks. Expanding the baseline is an
explicit scope decision. Preserve original gameplay and workflows; presentation
changes must not silently remove behavior.

## Current foundation

The repository has lossless save-container rewriting, partial typed snapshots,
deterministic primitives, clock/clear-landscape simulation, and selected object
callback bodies. These are tested against the pinned original implementation.
They do not establish complete world simulation, general mod support, playable
clients or multiplayer interoperability. See the [README](../README.md) and
[verification record](verification.md) for the implemented boundaries.

The stages below are the next roadmap, not retrospective claims that these
foundations complete a stage. The [stage 1 plan](stages/01-compatibility-contract.md) and
[versioned contract](../compatibility/contract.json) record the active concrete
matrix, fixtures and executable checks. Stage 1 was completed by the maintainer merge of
[PR #2](https://github.com/aron98/openttd-rust/pull/2), commit
`e35f0f7375d84124f8047adff404eb4273b7277b`. Stage 2 is ready for review in
[PR #3](https://github.com/aron98/openttd-rust/pull/3); its
[plan](stages/02-world-loading-saving.md) records the approved boundary and
passing implementation review and Linux CI evidence. Final documentation CI is
tracked in the PR checks before removing draft status. The maintainer's merge
remains pending; stage 3 is planned and has not started.

## Stages and acceptance gates

| Stage | Deliverables | Acceptance gate | Status |
| --- | --- | --- | --- |
| 1. Compatibility contract | Pin save, protocol, settings and content requirements; inventory missing behavior; define representative vanilla/modded fixtures, recorded commands, expected state and reproducible test procedures. | Every compatibility claim has a named scenario, observable result and verification procedure; baseline checks execute, unsupported contexts fail explicitly, and future scenarios are identified as unimplemented. | Completed |
| 2. Complete world loading and saving | Decode complete saved state, object pools, IDs, references and settings; restore content-independent derived state; serialize modified vehicles, companies, towns, industries, stations, orders, cargo and infrastructure; preserve mod/script state. | Populated worlds match saved state and the defined structural runtime state; supported edits survive Rust save/original reload and original save/Rust reload. Container preservation alone does not pass. Content-dependent runtime restoration is an explicit stage 4 gate. | Ready for review |
| 3. Commands and tick execution | Implement command validation, costs, execution and ordering; integrate callback dispatch, clocks and RNG into the world loop. | The engines replay supported command sequences with equal state at defined checkpoints, including pause, timing boundaries and save/reload. Unsupported gameplay remains explicit pending stage 4. | Planned |
| 4. Gameplay parity | Restore content-dependent runtime state, including NewGRF-derived vehicle, town, station and infrastructure caches; complete construction, terrain/water, vehicles/pathfinding, orders/service/breakdowns, cargo/stations, towns, industries, economy and company lifecycle in vertical slices, with applicable content/script behavior. | The stage's baseline gameplay inventory is covered by original-versus-Rust scenarios, including vanilla/modded worlds and save/reload during execution; no required gameplay family is silently deferred. | Planned |
| 5. Native mixed multiplayer | Complete negotiation, packet/command encoding, joining, scheduling, synchronization, reconnects and headless server operation. | Original and Rust clients run together on original and Rust servers, including late join, reconnect and save/reload, without desynchronization across the defined corpus. | Planned |
| 6. Desktop client | Shared renderer, sprites/palettes/text, audio, windows, tools, shortcuts and game management. | Original workflows are usable through the Rust client; representative visual/input checks and mixed multiplayer scenarios pass. | Planned |
| 7. Browser client | Wasm client, browser storage/content import, audio/input and multiplayer transport/gateway. | Browser, native Rust and original clients share games on both server implementations through the documented transports; simulation parity and browser save workflows pass. | Planned |
| 8. Compatibility hardening | Extended sessions, large worlds, mod combinations, malformed input, performance, cross-platform determinism and historical save coverage. | Publish a versioned compatibility matrix supported by repeatable tests and extended mixed-session evidence, with remaining limits stated explicitly. | Planned |

The maintainer approved the stage 2 boundary refinement on 2026-10-09:
complete saved-state loading/editing and structural restoration belong to stage 2;
content-dependent runtime restoration belongs to stage 4. This changes sequencing,
not the final compatibility goal. Stage 2 must not claim full gameplay-ready or
NewGRF-derived runtime parity.

Stages proceed through the [development workflow](../CONTRIBUTING.md). Only the
current stage is implemented. Each stage has one PR containing atomic commits;
the maintainer's merge is the gate to the next stage. Large stages are decomposed
into internal milestones, not silently narrowed to the first successful slice.
Any change to stage boundaries or acceptance criteria must be agreed before it
is used to declare completion.

## Work that spans stages

These concerns shape each active stage. They do not authorize starting a later
stage while the current PR is awaiting merge.

- **Content compatibility:** existing NewGRFs must retain identity, parameters,
  load order, evaluation/callback semantics and persistent state. Build modded
  fixtures alongside the gameplay systems they exercise. Include base graphics,
  sound/music, scenarios, heightmaps, dependencies and content acquisition.
- **AI and GameScripts:** preserve existing Squirrel scripts, versioned APIs,
  scheduling/execution budgets, events, commands and save/load behavior. Design
  and test the compatible runtime alongside the systems it depends on. A new
  scripting language is not a substitute for existing-script compatibility.
- **Networking:** stage 1 defines the four client/server combinations below and
  protocol probes; later active-stage plans may include prerequisite handshake
  or limited-world checks. General mixed multiplayer remains the stage 5 gate.
  Original clients and servers must work without being patched for our tests.
- **Browser portability:** establish an early Wasm compilation and deterministic
  test path within an applicable active-stage plan. Keep filesystem, networking,
  clocks and rendering behind host interfaces. A playable browser client is
  still stage 7; native-only dependencies must not silently foreclose it.
- **Evidence:** maintain independent native comparisons and meaningful negative
  controls throughout. Do not weaken comparisons, disable synchronization
  checks or treat an isolated supported scenario as full compatibility.

## Required multiplayer matrix

| Server | Original 15.3 client | Rust native client | Both together |
| --- | --- | --- | --- |
| Original 15.3 | Reference baseline | Required | Required |
| Rust | Required | Required | Required |

Stage 7 extends each applicable scenario to include the browser client. Content
versions and parameters must match the session. Tests cover scheduled commands,
RNG/state consistency, late joins, reconnects and persisted continuation, not just
successful connections. OpenTTD uses local deterministic simulation plus ordered
commands; see the [pinned upstream multiplayer description](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/docs/desync.md).

## First integrated gameplay milestone

Across stages 2 and 3, reach: load a small populated world, execute a supported
construction command in Rust, save it, and continue it in original OpenTTD.
Stage 2 establishes the world/save prerequisites; stage 3 adds the command.
This is a useful intermediate proof, not a replacement for either stage's gate.
