# Native deterministic replay protocol

`reference/replay-format.json` is the authoritative example input. Actions execute
in strictly increasing ordinal order, independent of the saved tick counter.
`command` contains `request` (company u8, mode `post` or `estimate`, tagged
`command`); `tick` contains count u32; `checkpoint` contains a unique safe label.
The native runner writes `initial` and `final` checkpoints automatically.
User checkpoint labels must therefore differ from these reserved labels.
Maximum actions: 10,000; cumulative state-loop calls: 100,000.

Command tags and arguments:

- `build_road`: tile u32, pieces u8, road_type u8, toggle_disallowed u8, town_id u16.
- `landscape_clear`: tile u32.
- `increase_loan` / `decrease_loan`: method u8 (interval=0, max=1, amount=2), amount i64.
- `rename_company` / `rename_president`: text string.
- `pause`: mode u8 (native bit index), paused bool.

Each native action observation has ordinal, op, before/after runtime state.
Commands additionally have `receipt` with `posted` bool, `gate` null/`tile`/`pause`,
and nullable `test`, `exec`, `result` costs. A cost has success bool, signed cost,
expenses u8, nullable native error symbol, and numeric error_params. Generic native
CMD_ERROR uses `CMD_ERROR` rather than an invented localized message. Supplemental
native_metadata contains numeric error/extra-error IDs and owner for each phase.
Native test costs are captured before affordability validation; final cost can
therefore differ. An invalid company produces final CMD_ERROR without a test.
Pause's NoEst flag causes execution even when mode is estimate.

Each checkpoint has `.sav`, `.world.json`, `.schema.json`, `.derived.json`,
`.runtime.json`. Saved descriptors and defined structural state describe the same
boundary. Runtime records before/after synchronous save so save lifecycle effects
remain visible. `results.json` lists all action observations and checkpoints.
Native runtime metadata includes interactive RNG and current company for audit;
these host-local values are not saved deterministic state. Gameplay RNG is.

The replay cursor and pending actions are external scheduling state, not native
save chunks. A replay envelope must preserve the remaining ordered action list
and next ordinal alongside its checkpoint save; restart replays that suffix after
fresh native load. Comparison treats after-load as a distinct lifecycle boundary.

The instrumented runner hooks StateGameLoop only to take ownership once after
load; requested ticks call the unchanged original body with all native registered
callbacks active. Host realtime, window/input servicing, and network transport are
outside this deterministic runner. No general gameplay or networking parity is
claimed by the protocol alone.

## Fixture preparation

The native-only `fixture` field is a preparation recipe, not a replay action and
not accepted by the Rust replay API. Run prepare recipes against the original
fixtures, then run `empty.json` against their output to prove complete saved-field
reload equality before using the prepared saves for differential execution.

- `prepare-populated.json` starts from `fixtures/world/populated-v362.sav`. It
  finishes due linkgraph jobs through original `JoinNext`, stops AI/GS via native
  APIs, keeps populated world pools, makes companies human with explicit names,
  and pauses. Original job completion is necessary: a due job otherwise adds a
  LinkGraph pause bit at load, creating an unsupported lifecycle dependency.
- `prepare-clear.json` starts from `fixtures/generated-v362.sav`. Original pool
  cleanup removes active gameplay objects; native tile accessors author high flat
  clear/void terrain. One native Town is required because the original rejects
  normal-mode saves with zero towns. Its custom growth is disabled, there are no
  houses/roads, and monthly bookkeeping remains active. One human company and
  vanilla year 2100 support bounded finance/clock callbacks. Settings and counters
  disable or bound unsupported events; actual replay retains every callback.

The initial generated input has no company: original after-load creates a human
company and advances gameplay RNG. That setup effect is retained in the prepared
fixture. Prepared fixture reloads must compare all saved fields without filtering
RNG, pause bits, GLOG, or any object pool. Source/binary freshness stamps cover every
instrumentation patch/header and the executable; the native driver rejects a
missing or stale stamp.
