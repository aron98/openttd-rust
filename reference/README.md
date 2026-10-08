# Runtime snapshot reference

This is instrumentation for the OpenTTD revision pinned in `upstream.toml`
(15.3). `snapshot.patch` inserts hooks into the original native save handlers;
`snapshot.hpp` is GPL-2.0-only reference instrumentation using OpenTTD's bundled
nlohmann JSON implementation. OpenTTD and its original descriptors/functions are
copyright their respective OpenTTD contributors; see the pinned source's COPYING.

The helper reads original in-memory SaveLoad descriptors for DATE and PATS,
original Tile accessors for every raw tile field, and original script Randomizer
objects for SRND. It does not parse a save file. Begin/Finish surround the
synchronous `SlSaveChunks` call, so the emitted JSON describes the same boundary
as the native output (before optional asynchronous compression). Unsupported
runtime descriptor types throw rather than silently omitting fields. All current
DATE and PATS fields are included; bool descriptors emit numeric 0/1.

Build in a separate ignored source/build tree (never modifies `.reference/OpenTTD`
or `.reference/build`):

```sh
scripts/setup-snapshot-reference.sh
```

For an existing local source/assets cache, set `REFERENCE_SOURCE` to the pristine
checkout and `REFERENCE_OPENGFX_ARCHIVE` to `opengfx-7.1-all.zip`. The exact source
commit and graphics SHA-256 are checked. `JOBS` controls build parallelism.

Run from the repository root with a fresh output directory:

```sh
cmake -DORACLE="$PWD/.reference/snapshot-build/openttd" \
  -DRUN_DIR="$PWD/.artifacts/snapshot-native-16" \
  -DCONFIG="$PWD/scripts/reference.cfg" \
  -DINPUT="$PWD/fixtures/generated-v362.sav" -DTICKS=16 \
  -P scripts/run-snapshot-reference.cmake
cmake -DRUN_DIR="$PWD/.artifacts/snapshot-native-16" \
  -P reference/check-snapshot.cmake
```

`INPUT=GENERATE` creates a seeded world. Historical fixtures are migrated by the
original engine before saving. Each run produces `snapshot.json`,
`primitives.json`, `save/autosave/exit.sav`, and stdout/stderr logs. The snapshot
runner includes the original reference runner's nonzero-exit, save/load-log,
expected-load-count and native-save checks: upstream failure with exit status
zero is still rejected. Stale saves/snapshots are rejected. Execution is bounded
by the original runner's 60-second timeout.

`primitives.json` comes from actual upstream Randomizer::Next/Next(limit), calendar
conversion methods, and TileXY/TileX/TileY calls. It includes seeds 0, 1, UINT32_MAX
and an unequal restored state; unbounded and bounded RNG transitions; leap-year,
century and maximum-year dates; current-map coordinate edges. Probe randomizers
are local, calendar/coordinate functions are pure, and no simulation state is
modified. Month values are zero-based, days one-based.

The structural check validates schema, native save presence, tile count and
current DATE/PATS/SRND coverage. The Rust differential comparison is a separate
check; structural success alone is not a semantic parity claim.

The checked-in `primitives.json` was emitted by the instrumented executable from
`fixtures/generated-v362.sav` at TICKS=1 using the command above. Its 64x64 map
probes are tied to that fixture; RNG/calendar probes are fixture independent.
JSON whitespace was formatted after generation; values were not edited.

## Gameplay subsystem probes

`gameplay.hpp` executes the original pinned engine's calendar, economy and tick
`TimerManager::Elapsed` routines and `RunTileLoop` (which dispatches to the original
clear/void tile procedures). It contains scenario setup and capture, not replacement
simulation algorithms. `gameplay.patch` exposes the reference build's timer
registries so ordinary object callbacks can be removed and event-only observers
registered. The original timer and tile-loop function bodies remain unchanged.
The setup script applies the two independent patches idempotently, including an
upgrade from an existing snapshot-only checkout, then verifies tracked source
against exactly the pinned revision plus those patches.

```sh
cmake -DORACLE="$PWD/.reference/snapshot-build/openttd" \
  -DRUN_DIR="$PWD/.artifacts/gameplay-native" \
  -DCONFIG="$PWD/scripts/reference.cfg" \
  -DINPUT="$PWD/fixtures/generated-v362.sav" \
  -P scripts/run-gameplay-reference.cmake
```

The runner sets `OTTD_GAMEPLAY_PROBES_PATH`. At the native save boundary, this
explicit probe mode runs an expendable process: it writes and closes the fixture,
then uses `std::_Exit(EXIT_SUCCESS)` without completing the save or running global
destructors against the deliberately modified timer registries. It produces no
native exit save and must not be used as a gameplay save runner. With this variable
absent, the existing snapshot/save path is unchanged. The runner rejects stale
probe output, native failures, load errors, missing loads, missing output, and
incomplete top-level case collections.

`gameplay.json` is the unedited JSON emitted by this command. Its 15 clock cases
capture all clock fields, cached year/month values, per-tick return values, ordered
calendar/economy boundary events, and gameplay RNG before/after. Cases cover day,
leap/non-leap February, quarter/year boundaries, frozen/slowed calendar time,
wallclock time, paused calls, tick overflow, and continuation across both maximum
calendar/economy years. Maximum wallclock-year date adjustment intentionally keeps
the original engine's cached year/month behavior.

Four landscape cases capture every raw map field, clock state, tile-loop cursor,
gameplay RNG and every tick's ordered events. They cover 64x64 and 128x64 maps,
the zero tile's scheduling boundary, complete 256-tick scheduler cycles, grass
counter/density growth through 6144 ticks, paused operation, and tick overflow.
Unused raw fields contain sentinels. The JSON document under each `before` and
checkpoint `after` is directly usable by the Rust subsystem simulator. Checkpoint
`ticks` and recorded events are cumulative from the case's initial state.

This is deliberately a **subsystem oracle**, not a full game simulation: normal
mode, temperate grass/rough/rocks and void tiles, no snow/fields/object pools,
no ordinary timer callbacks, and no ambient NewGRF callbacks. The harness selects
whether to advance for paused probes; it does not test the engine's UI pause
control. Synthetic terrain uses non-freeform edges so tile zero is an active clear
tile. Clock-only maximum-year cases may adjust inherited object dates internally;
the expendable process isolates those changes from ordinary snapshots and saves.
