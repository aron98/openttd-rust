# Town funding counter — increment design

This is the lead-level design for the first bounded slice of the
`gameplay.towns-industries-economy` stage gate (stage doc "Scope and
completion gates"). Today the whole tick domain refuses every funded town at
admission:

```
unsupported("town", "growth must be custom-disabled and unfunded")
```

(`crates/ottd-sim/src/world_ticks/domain.rs:134-150`, one conjunction:
`flags & 9 != 8 || growth_rate != 65535 || fund_buildings_months != 0`),
and `crates/ottd-sim/src/world_ticks/towns.rs` decrements
`road_build_months` but never `fund_buildings_months`. No existing capture
exercises funding: all six stage-4 cases have `growth_rate 65535`,
`fund_buildings_months 0`.

## Native authority (pinned 14ec60f)

- `src/town.h:34` — `TOWN_GROWTH_RATE_NONE = 0xFFFF`.
- `src/town_cmd.cpp:4116-4143` — `_economy_towns_monthly`
  (`{MONTH, Priority::TOWN}`), per town in `Town::Iterate()` order:
  `road_build_months--` (`:4120`) **then** `fund_buildings_months--`
  (`:4121`), before exclusivity/unwanted/history work; `UpdateTownGrowth`
  (`:4138`) then `UpdateTownRating` (`:4139`).
- `src/town_cmd.cpp:3863-3870` — `UpdateTownGrowthRate` returns immediately
  for `TownFlag::CustomGrowth` towns: `growth_rate` is never recomputed.
- `src/town_cmd.cpp:3876-3912` — `UpdateTownGrowth` for a funded
  custom-disabled town is a deterministic no-op: the setting gate at `:3883`
  does not return while `fund_buildings_months != 0`; the goal check is
  skipped by `:3885`; the `CustomGrowth` branch at `:3902-3905` returns
  without setting `IsGrowing` because `growth_rate == TOWN_GROWTH_RATE_NONE`;
  the `Chance16(1, 12)` at `:3908` is unreachable. No RNG is consumed and no
  observable state changes beyond `IsGrowing` staying clear.
- `src/town_cmd.cpp:3552-3577` — `TownActionFundBuildings` (via
  `CMD_DO_TOWN_ACTION`): requires `economy.fund_buildings`; on execute sets
  `fund_buildings_months = 3`, calls `UpdateTownGrowth`, and clamps
  `grow_counter` to `min(grow_counter, 2 * TOWN_GROWTH_TICKS -
  (growth_rate - grow_counter) % TOWN_GROWTH_TICKS)`. This executes during
  native fixture preparation only; its effects ride into the capture through
  the saved checkpoints (including the company-cash mutation and any RNG
  draws), which Rust compares as world state. **The increment implements no
  command path and no `UpdateTownGrowth` code** — the funded monthly walk is
  the counter decrement only.

## Saved-state footprint

`fund_buildings_months` and `grow_counter` are saved `CITY` fields already
restored and exported by the observer's `world.json` CITY chunk. The
increment requires **no `ottd-save` changes**: the funding action happens
native-side during fixture preparation; Rust needs only (a) admission of
funded custom-disabled towns and (b) the monthly decrement in native order.

## Evidenced fixture facts and capture plan

The replay runner's `actions.json` vocabulary has no town-funding prepare
kind today. Extend it (Slice A) with one additive whitelisted prepare kind
posting `Command<CMD_DO_TOWN_ACTION>::Post` with the fund-buildings action,
mirroring the `industry_recipient_prepare_buy_rights` precedent
(`reference_replay_commands.hpp:42-48`). The seeds already carry
`economy.fund_buildings = 1` and `town_growth_rate = 0`.

Add a new fixture case `fixtures/stage04-handoff/town-funding/`
(`input.sav` cloned from an existing single-town seed, `openttd.cfg` copied,
new `actions.json`) that funds the town at an early tick and runs ~4 economy
months of monthly checkpoints (well under the 100k-call limit), so `CITY`
diffs show `fund_buildings_months 3 → 2 → 1 → 0` alongside the
already-implemented monthly fields. All seven cases are then captured with
the rebuilt observer into a fresh run root
(`.omo/evidence/stage04-remote-run-03`; run-01/02 retained untouched). The
six existing cases must remain byte-identical in every compared channel —
the observer patch is additive-only. The new binary SHA-256 is recorded per
protocol, the README verification block and fixture-bundle checks are
extended additively (18 → 21 exact inputs), and the six existing scenario
tests are re-run against run-03 before any Rust funding code is admitted.

Contract accounting: if the new case requires a new scenario declaration or
extended bundle policy, changes must be additive; if any validator turns out
to hard-pin the 143-scenario count against extension, stop and escalate to
the lead rather than weakening or folding policies.

## Increment boundary

### Slice B — Rust counter implementation

- `world_ticks/towns.rs` monthly: add the `fund_buildings_months` decrement
  immediately after the `road_build_months` decrement, in native order.
- `world_ticks/domain.rs:134-150`: delete **only** the
  `fund_buildings_months != 0` clause from the admission conjunction.
- Still refused, unchanged: `growth_rate != 65535` (`CmdTownGrowthRate` is
  GameScript-only), `road_build_months != 0` (roadworks tile mutation,
  `road_cmd.cpp:2039-2054`), the `economy.town_growth_rate` setting != 0,
  all real growth (`IsGrowing`, `grow_counter` advancement, `GrowTown`
  house/road placement), and the `GetNormalGrowthRate` family
  (`town_cmd.cpp:3836-3857`: station counts, house-count cache). The
  `grow_counter` clamp of the funding action remains uncredited (native
  fixture prep only).

### Slice C — scenario guarantees

The new town-funding scenario test must prove exact `CITY` parity across
the funded interval (fund counter 3→2→1→0, already-implemented monthly
fields, unchanged `growth_rate`/`IsGrowing`-clear flags) against genuinely
regenerated run-03 evidence, with `check_one` semantics (exactly 1 passed /
0 failed / 0 ignored). A negative control must prove that a town with
`growth_rate != 65535` (or any remaining refused clause) still refuses
admission with the unchanged error.

## Dependency verdict

Independent of the script atoms and of all wave-3 files (touches only
`world_ticks/*` plus the observer prepare vocabulary). The observer patch
and shared build are owned by exactly one worker for the wave; the lead
schedules the rebuild so no other worker's instrumented source or binary is
overwritten. Pairs best with `crates/ottd-script` atom 4 (fully disjoint).

## Verification gates (unchanged baseline rules)

Contract validate, manifest pin walk, cargo fmt/clippy/test --workspace
--locked, pytest under pinned Python 3.11, unittest discover, README
verification block for the recaptured run root, and the exact scenario
tests (existing six re-run against run-03 plus the new case; check_one each).
All seven stage gates remain open; this increment closes no broad gate.
