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
