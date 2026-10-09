# Native deterministic replay protocol

`reference/replay-format.json` is the authoritative example input. Actions execute
in strictly increasing ordinal order, independent of the saved tick counter.
`command` contains `request` (company u8, mode `post` or `estimate`, tagged
`command`); `tick` contains count u32; `checkpoint` contains a unique safe label.
The native runner writes `initial` and `final` checkpoints automatically.
Rust checkpoint labels must be unique ignoring ASCII case and differ from the
reserved `initial`/`final` labels ignoring case, so publication is portable across
case-sensitive and case-insensitive filesystems. Labels use ASCII letters, digits,
underscore and hyphen.
Maximum actions: 10,000; cumulative state-loop calls: 100,000.

Command tags and arguments:

- `build_road`: tile u32, pieces u8, road_type u8, toggle_disallowed u8, town_id u16.
- `landscape_clear`: tile u32.
- `terraform_land`: tile u32, slope u8 (raw corner mask), dir_up bool.
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

TerraformLand receipts additionally carry `returns` with nullable `test`, `exec`,
and `result` entries. Each entry is `{kind: "landscape", additional_money: i64,
tile: u32}` captured from the native command tuple. This preserves the distinction
between a body error's invalid tile, an invalid-company default tile zero, and a
successful test tuple retained by a later affordability failure. Cost-only command
receipts keep their existing shape.

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

## Full comparison matrix

After `scripts/setup-snapshot-reference.sh` and `cargo build --locked -p ottd-cli`:

```sh
bash scripts/check-replays.sh
# Or select an explicit fresh directory:
python3 -m scripts.check-replay --artifacts "$PWD/.artifacts/replay-new-run"
```

The output directory must be new. `--case NAME` runs an explicitly partial named
scenario and marks `full_matrix` false; the full invocation also runs resume,
original continuation and negative controls. Native and Rust action receipts,
deterministic runtime fields, every saved descriptor/raw map byte, defined
structural state, and the Rust output saves decoded again must agree. Only the
native host observers `interactive_random`, `current_company` and supplemental
numeric error metadata are absent from the Rust results comparison; saved state
is always compared without field filtering.

`cases.json` retains source recipes and hashes for boundary and branch fixtures.
They cover month, quarter, year, leap day, u64 tick wrap, growing grass across
4096 ticks, nonempty town history and monthly counters, insufficient funds,
pause admission, construction-limit refill while paused, and nested president
renaming. The ordinary command corpus exercises successful, estimated, duplicate,
invalid, ownership and ordering-sensitive calls.

`construction-continue.json` executes road construction and unpauses without
requesting unsupported populated Rust ticks. Its Rust output is loaded by the
uninstrumented original for 16 null-driver iterations; the witness requires a
strictly increased saved tick counter and preservation of the constructed road.
This is original continuation evidence, not populated Rust gameplay parity.

The shell entry point checks the native build stamp, builds the CLI with `--locked`,
prints `Artifacts: <directory>`, and reports `PASS replay checks` only after the full
matrix passes. `OTTD_REPLAY_ARTIFACTS` selects a fresh explicit directory; default
outputs are `.artifacts/replays-*/results`; the announced default artifact directory
is the parent `.artifacts/replays-*`, with evidence paths relative to its `results/`
child. An explicit `OTTD_REPLAY_ARTIFACTS` is the actual runner output directory
and is announced directly. Binary overrides are
`OTTD_REPLAY_ORACLE`, `OTTD_REPLAY_ORIGINAL_ORACLE`, and `OTTD_REPLAY_CLI`.

Three numeric edge cases retain independently observed original behavior:
`large-loan` exercises saturated Money multiplication before monthly interest
division; `loan-command` exercises saturated explicit-loan addition; `town-sum`
exercises the native signed 32-bit accumulator and unsigned conversion when
averaging filled town histories. Their native preparation recipes author inputs;
expected costs and checkpoints come exclusively from original command/loop calls.

The eleven `terraform-*` cases prepare fresh paused populated worlds from
`populated-v362.sav` using their corresponding `prepare-terraform-*` recipes.
Native preparation authors a checked clear-terrain pad, valid freeform borders,
an optional legal height-limit cone, or an actual command-built tunnel. It rejects
overlapping vehicles and infrastructure. Costs and expected results come from
actual native TerraformLand calls, including recursion, raw masks, limits,
affordability, command gates, void borders and tunnel obstruction. Every checkpoint
compares complete saved state and derived/runtime observations. Other tile
procedures and LevelLand partial success remain outside this increment.

Selecting `terraform-basic` also runs tuple, cost, height and command-order failure
controls. Selecting `terraform-resume` runs fresh-process split/resume equality
and an uninstrumented original sixteen-tick continuation that preserves the edited
height. Summary fields `terraform_controls` and
`terraform_resume_and_continuation` record these checks independently of the
legacy full-matrix controls. No populated Rust tick parity is claimed.
