# Vehicle-year unprofitable warning — increment design

This is the lead-level design for the annual-warning seam named in
`docs/stages/04-remote-handoff.md`: the native advances the final world-tick
call 27009 of the annual company/vehicle rating horizon, whose vehicle-year
loop includes an unprofitable-vehicle warning (news item plus AI event
context) that Rust refuses today with

```
unsupported world tick phase vehicle_year: call 27009: unprofitable warning/news and AI event context
```

(`crates/ottd-sim/src/runtime/movement/year.rs:9-15`, surfaced through
`crates/ottd-sim/src/world_ticks/plan.rs:66-78` call numbering). The passing
annual-warning scenario proves the refusal preserves complete committed state
and worker leases; it is rollback evidence, not warning evidence.

## Native authority (pinned 14ec60f)

`src/vehicle.cpp:3010-3035` — `_economy_vehicles_yearly`,
`IntervalTimer<TimerGameEconomy>` at `{YEAR, Priority::VEHICLE}`. Per primary
vehicle, in `Vehicle::Iterate()` order:

1. `profit = v->GetDisplayProfitThisYear()` (`raw >> 8`, `vehicle_base.h:572`).
2. If `v->economy_age >= VEHICLE_PROFIT_MIN_AGE (730, vehicle_func.h:37)` and
   `profit < 0`:
   - News: if `_settings_client.gui.vehicle_income_warn && v->owner ==
     _local_company` — `AddVehicleAdviceNewsItem(AdviceType::
     VehicleUnprofitable, encoded(STR_NEWS_VEHICLE_UNPROFITABLE_YEAR |
     _PERIOD by `TimerGameEconomy::UsingWallclockUnits()`), v->index)`
     (`news_func.h:40-43`, `news_gui.cpp:900-913`; `std::list _news` front
     insert, trim at `MAX_NEWS_AMOUNT = 1<<10`).
   - AI event: `AI::NewEvent(v->owner, new ScriptEventVehicleUnprofitable(
     v->index))` (`ai/ai_core.cpp:235-253`). For non-AI owners the event is
     dropped before any storage (`IsValidAiID` requires `c->is_ai`,
     `company_base.h:150-154`) and the `ScriptObjectRef` guard frees it; no
     VM is involved in queueing, and consumption happens on later ticks only.
3. `v->profit_last_year = v->profit_this_year; v->profit_this_year = 0;`
   then `GroupStatistics::UpdateProfits()` and window invalidation.

Ordering inside the economy-year trigger (`timer_game_economy.cpp:125-180`):
COMPANY (`company_cmd.cpp:811-830` expense rotation) → TOWN
(`town_cmd.cpp:4145-4157` house aging) → VEHICLE (this loop). Company
rating/quarter work is month/quarter priority and precedes the year timers in
the same call. The annual-warning capture confirms this sequence
(`rating.jsonl` sequences 86-95 before `year.jsonl` sequences 2-9, all at
tick 81029).

## Saved-state footprint

None. There is no news chunk in the saveload chunk-handler table
(`saveload/saveload.cpp:261-298`) and no script-event-queue chunk
(`saveload/ai_sl.cpp:75-168` stores only name/settings/version plus the
script's own `Save()` data; `GSDT` analogous). `ScriptStorage::event_queue`
is dropped across save/load. The only saved mutations of the warning path
are the vehicle profit rotation fields (`saveload/vehicle_sl.cpp:752-755`)
and `economy_age` (`:729`), all already restored. `vehicle_income_warn` is a
client setting, not in the save. **Therefore the Rust increment requires no
`ottd-save` changes at all**; news/AI-event state is process-owned runtime
state (precedent: `crates/ottd-sim/src/runtime/company_rating.rs:9-40`,
which is likewise process-owned and never serialized).

## Evidenced fixture facts

`fixtures/stage04-handoff/annual-warning/` — human company 0, single primary
road vehicle, no AI, no GameScript. In the final call (tick 81029): the news
branch condition provably held (`vehicle_income_warn: true`, `owner ==
local_company == 0`, `economy_age 1094 >= 730`, display profit
`-287985 >> 8 = -1125 < 0`) and the AI branch was a provable drop
(`is_ai: false`). Neither effect appears in any saved chunk or exported
channel, and the string encoding consumes no RNG — the warning work is
invisible to every existing comparison. A sweep of all six stage-4 case
captures (2026-10-12) confirms the unprofitable branch fires in
`annual-warning` only; the other five cases never meet the condition.

## Increment boundary

### Slice A — observer extension first (any implementation credit requires it)

The warning is currently an oracle gap: no probe sits inside the
`vehicle.cpp:3016-3023` branch. Extend `reference/vehicle_year.patch` with
additive probes (one owner per wave; the lead schedules this shared build):

- `unprofitable_news` event in the news branch (vehicle index, encoded
  string id, display profit) and `unprofitable_ai_drop`/`unprofitable_ai` in
  the AI branch, emitted via `ReferenceGroupProfit::Event` between
  `vehicle_year_enter` and `group_profits_before`.
- Snapshot fields in `reference_group_profit_snapshot.hpp` for news-queue
  length and (if cheaply reachable) per-company script-event-queue length.

Because the snapshot schema is shared, all six cases are recaptured into a
fresh run root (e.g. `.omo/evidence/stage04-remote-run-02`); run-01 is
retained untouched. The new observer binary SHA-256 is recorded per protocol,
the README verification block must pass, and all six Rust scenario tests are
re-run against the new captures before any Rust warning code is admitted.

### Slice B — Rust warning implementation

Replace the blanket refusal in `runtime/movement/year.rs:9-14` with the
exact native branch structure inside the admitted single-primary-vehicle
movement domain:

- Per-primary-vehicle evaluation in native order, profit rotation and group
  recount unchanged.
- News branch gated on `(vehicle_income_warn && owner == local_company)`,
  inserting an item into a **process-owned news queue** (never serialized,
  never fabricated in saved state). String variant selection
  YEAR vs PERIOD follows `UsingWallclockUnits()`; the PERIOD (wallclock)
  variant stays refused until evidenced.
- AI branch: exact native no-op for human owners (evidenced drop); AI owners
  remain refused until script-event storage is evidenced by a capture.
- Still refused, unchanged: multi-primary-vehicle movement domains, news
  trimming/`MAX_NEWS_AMOUNT` behavior beyond the single insert, news
  consumption (`NewsLoop`), `DeleteVehicleNews`/`ChangeVehicleNews`, and
  `GM_MENU` early-return semantics (no menu mode exists in the sim).

### Slice C — guarantee migration

The annual-warning scenario test (`company_rollback.rs::
genuine_post_quarter_refusal_preserves_world_ratings_and_worker_leases`) must
keep passing byte-identically until Slice B lands. After Slice B, the same
full 27010-call request must succeed against the regenerated native capture,
with the warning's news/AI effects checked through the new probe channel
(exact vehicle id, string id, profit; queue counters where exported), and
the explicit `news_queue_parity: false` record in
`crates/ottd-sim/tests/native_vehicle_year.rs` updated only against
genuinely regenerated evidence. The other five scenario tests must pass
against run-02 unchanged (their values must be identical).

## Dependency verdict

Independent of the script atoms. Queueing an AI event is a pure C++ enqueue
with no VM at production time; consumption is later-tick work. For the
evidenced configuration (human owner) the event is dropped before storage.
The idle `MaterializedIdle` session graph in `crates/ottd-script` is
unchanged by this increment.

## Verification gates (unchanged baseline rules)

Contract validate, manifest pin walk, cargo fmt/clippy/test --workspace
--locked, pytest under pinned Python 3.11, unittest discover, README
verification block for the recaptured run root, and the six exact scenario
tests (check_one: exactly 1 passed / 0 failed / 0 ignored each). All seven
stage gates remain open; this increment closes no broad gate.
