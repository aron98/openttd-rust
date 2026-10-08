# Compatibility foundation verification

## Typed snapshot milestone

Verified locally on macOS ARM64 on 2026-10-08 against the same pinned upstream
revision below. `cargo test --workspace --locked` passed 50 self-contained tests at initial integration;
the two external-oracle tests are explicitly run by `scripts/check-snapshots.sh`
instead of being counted as coverage from the ordinary test invocation.
Formatting and strict all-target/all-feature Clippy passed.

The differential run in `.artifacts/snapshots-RzmpVh/` passed all three fixtures
at 1 and 16 null-driver ticks. Each of the six directories retains the actual
C++ runtime `snapshot.json`, its paired `save/autosave/exit.sav`, two byte-identical
Rust snapshots, exact comparator results, typed-library parity results, and
primitive parity results. Tile, date, saved RNG and setting mutations all
returned nonzero with their exact paths and expected/actual values: 24 negative
controls total. No fields were filtered. The previous 12-case preservation matrix
also passed again in `.artifacts/compat-lFXq6R/`.

The new core tests replay actual upstream randomizer/scaling, calendar and map
vectors and check coordinate bounds, leap/century/max-year dates and checked date
arithmetic. The CLI additionally rejects duplicate JSON keys, inexact or
out-of-range numeric inputs, malformed JSON and byte-limit violations. Existing
rewrite no-clobber tests remain green. `snapshot` rejects historical versions;
the C++ engine performs the migrations in the differential fixture matrix.

This verifies typed map/clock/settings/randomness decoding and the deterministic
primitives. Simulation, game-object decoding, historical Rust migrations,
scripting, networking and UI remain unimplemented. CI now invokes both reference
matrices; the new CI configuration has only been validated locally at this stage.

## Earlier save-container milestone

Date: 2026-10-08. Platform: macOS ARM64. Rust: 1.96.0.
Upstream: OpenTTD 15.3 at `14ec60f248547d4d062a1160f0fc26d742319888`.
The GitHub releases API identified 15.3 as the latest stable release when pinned.

## Observed results

| Check | Result |
| --- | --- |
| Headless reference build, all four codecs | Built; `openttd -h` reports 15.3 |
| Strict Clippy, all workspace targets/features | Passed with `-D warnings` |
| `cargo nextest run --workspace --locked` | 23 passed, 0 skipped |
| `cargo test --workspace --doc --locked` | Passed; no doctests defined |
| CLI inspect/rewrite/help/error/no-clobber paths | Exercised through the executable |
| Shell syntax | Both shell scripts passed `bash -n` |
| Independent payload baselines | Original OTTN bytes or external XZ decompression matched Rust output exactly |
| Reference interoperability | Three fixtures x four formats passed (12 cases) |
| Negative oracle control | Failed load correctly rejected despite upstream exit code zero |
| Independent code review | CLEAR / APPROVE after all four findings were resolved |

The successful full matrix is retained locally in `.artifacts/compat-DkKvCM/`.
Each case includes the original independently decoded payload, Rust rewrites,
reference-engine logs, and exit saves. Successful comparisons excluded no chunks
or bytes. The generated fixture has a 64 x 64 map; the historical regression
fixtures contain more populated state. Their hashes and origins are in
`fixtures/README.md`.

## What the comparison proves

For each fixture, every compression rewrite preserves the original decoded
payload and loads in the original engine. For deterministic continuation, the
original engine first imports a fixture once. Both branches then start from that
shared native snapshot, run 64 null-driver ticks, and produce byte-identical
uncompressed saves after the Rust rewrite.

The engine executing both simulation branches is C++ OpenTTD. This demonstrates
that the Rust container rewrite preserves these inputs; it does not demonstrate
a Rust implementation of the simulation.

## Failures caught and fixed

- Initial tests ran against the empty codec/CLI and failed before implementation.
- A historical fixture exceeded the small synthetic-test byte budget; fixture
  tests now use the documented 256 MiB default without changing the limit tests.
- Independently importing older saves generated distinct random savegame IDs.
  Repeating the original-engine load reproduced the difference. Source:
  `src/saveload/afterload.cpp:3333` and `src/misc.cpp:87` at the pinned revision.
  One imported snapshot now provides identical starting state for both branches.
- Review exposed a false-positive oracle: failed loads fall back to the title
  world, save it, and exit zero. The negative case initially failed in
  `.artifacts/compat-bLJMmp/`. The runner now rejects upstream save/load error
  records and unexpected additional loads. That control passes in the final run.
- Review required an independent payload baseline; external `xz` and direct
  original-byte comparison now establish the expected bytes.
- XZ now uses bounded liblzma through xz2. The earlier candidate's dictionary cap
  did not also bound its index allocation. Both decoded bytes and codec memory
  have explicit limits; these are not a total process-memory ceiling.
- Randomized tests now reach chunk and compression parsing through valid headers,
  in addition to arbitrary whole-file input and structured round trips.

## Limits and environment notes

This milestone has no Rust gameplay, object-field interpretation, historical
state migrations, UI, scripting, or network implementation. It does not establish
compatibility for every old save or third-party content package. The full port
remains the product goal described in the README.

The GitHub Actions workflow was added but has not run on a remote runner. Local
execution validates macOS; Linux CI and Windows behavior are not claimed here.
The upstream build emitted a CMake CMP0177 policy warning and macOS deployment
target warnings for Homebrew libraries; it built and ran successfully. The Rust
checks passed without warnings. Editor LSP requests were intermittently cancelled;
the compiler, Clippy, tests, and real executable runs are the validation evidence.

These checks were performed locally before the initial repository publication.

## Clock and clear-landscape subsystem

Run `bash scripts/setup-snapshot-reference.sh` followed by
`bash scripts/check-simulation.sh`. The script starts a fresh native probe process,
then captures clock replay and CLI comparisons under `.artifacts/simulation-*`.
The native source and fixture provenance are documented in
[`reference/README.md`](../reference/README.md).

Coverage is 15 clock cases and 4 landscape cases with 22 checkpoints: 64x64 and
128x64 terrain, scheduling boundaries, recovery through 6144 ticks, pause and
u64 tick wrap. Every tile field, cached clock value, cursor, gameplay RNG and
ordered event record is compared. Repeated CLI output must be byte-identical.
Five deliberate mutations (tile, date, cursor, RNG, event) must exit unsuccessfully
and report the expected JSON path. Unit tests additionally reject unsupported
contexts, terrain, flood-prone void boundaries, malformed clocks and input limits.

This is isolated clock dispatch plus temperate clear-tile behavior, without timer
callback bodies, NewGRF callbacks, water simulation or game-object updates. The
positive-height invariant around void prevents the original void procedure from
leaving the supported terrain domain. These checks do not establish full-game
simulation parity or writing advanced state back to save files.
