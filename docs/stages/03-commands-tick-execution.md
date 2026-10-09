# Stage 3: commands and tick execution

Status: **In progress**. Branch: `stage/03-commands-tick-execution`, based on
the maintainer merge of [PR #3](https://github.com/aron98/openttd-rust/pull/3),
`251da64b1b0c9eb8de897ecd3dba81f2872972ca`. One stage PR will collect atomic
increments. The maintainer's merge remains the gate to stage 4.

## Goal and architecture

Run ordered, supported commands and actual deterministic world ticks against
the saved-world representation introduced in stage 2. The target remains
OpenTTD 15.3, upstream `14ec60f248547d4d062a1160f0fc26d742319888`, save362.
General gameplay bodies and content-dependent runtime restoration remain stage4.
Supported means an explicit validated domain, not permission to skip native
effects. A rejected replay publishes no partially advanced world.

`ottd-sim` may depend on `ottd-save`; the reverse dependency remains absent.
Commands and ticks operate on one authoritative `World`. Native failures are
command receipts; an unsupported Rust context is a distinct error. Save changes
must rebuild structural state through validated world operations. Host wall
time, UI processing and live network transport are outside this deterministic
runner; this does not establish mixed multiplayer.

The approved roadmap supplies the design and execution authorization. This plan
defines the concrete supported domain and evidence for that stage, preserving
the stage4 boundary. It does not replace general gameplay with fixture-only
success claims.

## Commands and ordered replay

Implement the native top-level test, affordability, execution and accounting
pipeline for these command bodies:

- Vanilla normal-road construction on flat clear or normal road tiles, including
  piece additions, duplicates, one-way restrictions and relevant ownership.
- Clearing supported clear ground, including costs, limits and adjacent water
  flag effects where required by the native operation.
- Loan increase and repayment by interval, maximum or explicit amount.
- Company and president naming, including native uniqueness/length rules and
  nested automatic company naming.
- Supported headless pause reason transitions and command-during-pause behavior.

`post` and `estimate` are distinct request modes. Native test/execute phases,
costs, expense categories, errors and early top-level gates are observable.
Estimation must retain its native affordability/pause semantics, including
`NoEst` commands such as pause that execute despite an estimate request. Do not invent
an execution receipt when the original returns at an earlier gate. Restore the
acting company at the same boundary as upstream. Construction uses native
vanilla price restoration, not hardcoded fixture prices. Unimplemented slopes,
tram/content-dependent road types, arbitrary command IDs and interactive pause
confirmation return explicit unsupported errors before committing state.

Replay actions have a monotonically increasing ordinal independent of the
saved simulation tick. Commands remain ordered while the game is paused.
Same-frame commands execute FIFO before the requested state-loop call, following
the ordering in `network/network_command.cpp` and `network.cpp`. Pending actions
and host scheduling position belong to a versioned replay checkpoint envelope;
they are not invented OpenTTD save chunks. Reject past/duplicate scheduling,
malformed input and excessive work at the public boundary. A split/reloaded
replay must preserve future actions and agree with uninterrupted execution at
the same native lifecycle checkpoint.

## Tick domain and native phases

A tick is one deterministic `StateGameLoop` call. The native reference keeps
the original timer registries and callback bodies active. The existing isolated
clock/landscape probes remain regression evidence only; stripping native
callbacks cannot prove this stage.

The initial unpaused domain is vanilla temperate clear/void terrain, with no
unsupported active vehicles, stations, industries, objects, animations,
scripts or linkgraph jobs. Settings, loaded counters and the requested horizon
must make unsupported creation/expiry events unreachable. Check this against
native state rather than assuming empty pools suppress all global work.
Native normal-game loading requires at least one town. Admit towns without
house tiles with custom growth disabled, and execute their native counters,
monthly ratings/history and yearly callbacks; do not bypass the native guard.
Human companies without unsupported assets exercise meaningful company
bookkeeping. Validate no active bankruptcy/HQ/assets, infrastructure maintenance
disabled, inflation disabled, steady economy, no subsidies and no tree spread;
read every required value from saved settings. Calendar dates after vanilla engine introduction/aging avoid
content-dependent engine transitions. Paused populated worlds have a separate
command path; a paused state loop still replenishes landscaping limits and
must reject active script work it cannot execute.

Integrate the native phase order: calendar clocks and boundary callbacks,
vehicle calendar phase where admissible, economy clocks and callbacks, tick
clock, tile scheduler, landscape/global counters, then company landscaping
limits. Account for saved RNG, industry daily counters, company cursor and
other active timers. Unimplemented industry construction, competitor creation,
disaster or asset behavior must cause transactional horizon rejection rather
than a skipped phase. Port the bounded human-company monthly finance/statistics
and yearly expense rollover needed for meaningful month/year acceptance cases.
World field/list operations needed for native history rollover must retain
wire and structural validation.

Replay load admission also requires an existing playable human company. Native
loading creates one and consumes RNG when none exists; that company-creation
lifecycle remains unsupported rather than being mistaken for a tick effect.

Preconditions will be recorded alongside the implementation and scenario
inventory. No arbitrary loaded save is declared safe to tick merely because
stage2 can decode it. Full vehicle/town/industry simulation remains stage4.

## Ownership and integration order

1. Commands owner: `ottd-sim/src/commands*` and command tests. Own command wire
   types, validation, native costs, bodies, nested-command accounting and
   receipts. Coordinate shared field access with the tick owner.
2. Tick owner: saved-world runtime modules/tests, `ottd-sim` dependency/exports
   and necessary validated save mutation APIs. Own phase dispatch, clocks,
   company callbacks, RNG/counters and transactional admission.
3. Native oracle owner: `reference/`, native fixture recipes, replay scripts and
   fixture data. Own original command/phase observation and complete checkpoint
   comparisons, without deriving expected native results from Rust.
4. Integration lead: freeze interfaces, serialize atomic commits, assign the
   replay CLI/contract/docs integration after the core interfaces stabilize,
   exercise real public commands and obtain independent review. No overlapping
   worker edits; use isolated worktrees if ownership cannot remain disjoint.

Internal milestones: source inventory and plan; command/tick interfaces;
command bodies with regressions; saved-world loop and callback integration;
native oracle and replay corpus; CLI and checkpoint envelope; compatibility
contract/CI; full review, corrections and stage PR handoff.

## Acceptance checklist

- [ ] Native command tests, costs, failures, execution accounting and estimates
      agree on complete saved-state checkpoints, including failed commands.
- [ ] Real road construction in a populated world changes map and company state;
      its Rust save loads and continues in unmodified original OpenTTD.
- [ ] FIFO ordering, acting company restoration, pause/unpause and independent
      replay scheduling agree; reordered-command negative controls fail.
- [ ] Admitted saved worlds execute native-equivalent complete state-loop calls
      at zero/one/multiple steps, day/month/year boundaries and counter wrap.
      Actual callback effects and RNG/counters agree, not only boundary labels.
- [ ] Unsupported state/events and malformed/oversized requests reject
      deterministically without partial state or output publication.
- [ ] Native/Rust checkpoints compare all saved fields/raw map data and the
      defined structural projection at matching temporal boundaries; estimates,
      failures and pause effects retain their actual native semantics.
- [ ] Save/reload and replay-envelope continuation preserve pending actions and
      match uninterrupted replay at equivalent native lifecycle checkpoints.
- [ ] Wrong cost, order, clock, RNG and saved-state controls fail through the
      real comparison path; missing/stale native evidence cannot pass CI.
- [ ] Formatting, strict Clippy, Rust/Python tests, existing native drivers,
      independent review and required PR CI pass. One PR awaits maintainer review.

## Verification and evidence

Native authorities: `command.cpp`, `command_func.h`, `company_cmd.cpp`,
`misc_cmd.cpp`, `road_cmd.cpp`, `clear_cmd.cpp`, `landscape.cpp`, `openttd.cpp`,
`timer/`, `economy.cpp`, and `network/network_command.cpp` in the pinned source.
Record source/patch/binary/input/action hashes, exact invocations, bounded exit
status, command receipts and every checkpoint save/JSON. Capture saved and
derived observations at the same boundary; native after-load is a separate
lifecycle checkpoint. Preserve original-only continuation witnesses without
claiming Rust parity for their unsupported gameplay ticks.

Baseline gates remain:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked
python3 scripts/check-contract.py --validate
python3 -m unittest discover -s scripts/tests -p 'test_*.py' -v
python3 scripts/check-contract.py --run baseline
```

Add the stage replay driver and its exact evidence paths to the versioned
contract before marking `commands.world-ticks` implemented. Extend PR-only CI
with scoped evidence retention; preserve the native compiler cache. Scenario
commands and artifact paths will be documented with the executable driver.
