# OpenTTD Rust

A Rust port targeting **OpenTTD 15.3**, with behavioral parity and two-way save
compatibility as the end goal. The exact upstream commit is recorded in
[`upstream.toml`](upstream.toml).

**Current state: typed snapshots, a deterministic clock/clear-landscape subsystem, and selected object callback bodies. This is not yet a playable game.**
The Rust code reads and rewrites save containers while preserving chunk contents.
Version-362 saves additionally decode into typed map, clock, settings and saved
random-state snapshots. Rust randomizer, map-coordinate and Gregorian calendar
primitives are checked against independently generated C++ vectors.
An explicit subsystem fixture can advance calendar/economy clocks and temperate
clear terrain against original C++ results. A separate typed-state runner executes
selected vehicle, house, company, station, and industry callbacks. Full object simulation, rendering,
scripts and multiplayer are not implemented.

## Use

```sh
cargo run -- inspect fixtures/generated-v362.sav
cargo run -- snapshot fixtures/generated-v362.sav > /tmp/world.json
cargo run -- compare /tmp/upstream.json /tmp/world.json
cargo run -- rewrite fixtures/generated-v362.sav /tmp/openttd-roundtrip.sav --compression lzma
cargo run -- --help
```

`inspect` prints JSON containing the save version, compression, and chunk framing.
`rewrite` supports `none`, `zlib`, `lzma` (XZ), and `lzo`. Omitting `--compression`
preserves the source format. The destination must not exist; writes are published
only after successful decoding and encoding. Versions and all chunk bodies are
preserved, including unknown chunks. An unknown chunk may still be rejected by
OpenTTD itself: this tool checks container structure, not gameplay validity.

`snapshot` emits schema-version-1 JSON for save version 362 only. Every tile is
listed in linear index order, preserving raw fields; DATE, PATS and SRND retain
their saved fields. Older saves require migration by upstream before typed
decoding. `compare` checks every JSON field, including unknown fields, and reports
the first mismatching path with expected and actual values. Duplicate keys,
non-integer numbers and numbers outside the exact i64/u64 domain are rejected.
Both JSON inputs are bounded by `--max-bytes`.

The default encoded/decompressed byte limit is 256 MiB; change it with
`--max-bytes`. This is not a total process-memory cap. The XZ decoder separately
limits its memory to 256 MiB. There is a 4096-chunk structural limit.

The header reader accepts versions 1 through 362 except upstream's reserved
patchpack range 220 through 286. Real interoperability has been checked for the
three fixtures at versions 211, 308, and 362. Version-zero/headerless saves,
original TTD saves, patchpacks, future save versions, and historical semantic
migrations are not implemented by Rust yet. A rewrite never upgrades a header
without migrating its state.

## Compatibility contract

The [versioned scenario contract](compatibility/contract.json) separates Rust
behavior, lossless container preservation, original-only baselines and future
requirements. Use Python 3.11+ to validate input hashes and scenario references
without running a game, or execute the existing checks with fresh reports:

```sh
python3 scripts/check-contract.py --validate
python3 scripts/check-contract.py --list
# After both reference setup scripts below:
python3 scripts/check-contract.py --run baseline
```

See the [contract guide](compatibility/README.md) and
[stage 1 plan](docs/stages/01-compatibility-contract.md). Native NewGRF/AI and
prejoin protocol probes are reference baselines; they do not establish Rust mod
execution or multiplayer support.

## Verify

Rust 1.96.0 is pinned in `rust-toolchain.toml`. The XZ dependency uses `liblzma`
through `xz2`; its build can use a system library or compile the bundled library.
A C toolchain is therefore required in addition to Rust.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --locked
```

To run interoperability tests, install CMake, a C++20 compiler, Git, curl, unzip,
XZ tools, and the upstream compression dependencies (zlib, liblzma, LZO).
On macOS these are available through Xcode Command Line Tools and Homebrew; on
Linux use your distribution's development packages. Then run:

```sh
bash scripts/setup-reference.sh
bash scripts/check-compatibility.sh
bash scripts/setup-snapshot-reference.sh
bash scripts/check-snapshots.sh
bash scripts/check-simulation.sh
bash scripts/check-callbacks.sh
```

Pull-request CI caches native compiler results while still building and running
every interoperability check. A separate job warms the shared cache only after
a PR is merged into `main`; neither workflow runs on pushes. See
[reference-build caching](docs/ci.md) for cache keys, local opt-in settings and
validation limits.

Setup builds the pinned original in `.reference/` and downloads a checksummed
OpenGFX 7.1 base set. It does not install a game globally. The compatibility
script uses isolated directories under `.artifacts/`, retaining engine logs and
resulting saves. It checks:

1. Rust output against the original uncompressed bytes, using external `xz` for
   compressed fixture baselines.
2. Every fixture rewritten in each of four formats, then loaded and saved by
   the original engine.
3. A shared native snapshot run for 64 null-driver ticks before and after Rust
   rewriting, with byte-identical resulting saves and no ignored fields.

Historical saves are first migrated once by upstream for the shared snapshot.
OpenTTD creates a random savegame ID when independently migrating older saves;
using one migrated starting state makes the comparison meaningful. These checks
prove preservation for the corpus, not full compatibility or Rust simulation.

The separate snapshot reference build applies the recorded instrumentation in
`reference/` to an isolated upstream checkout. The differential harness compares
the paired native save and C++ runtime snapshot for all three fixtures at 1 and
16 null-driver ticks. It checks repeatable Rust output and deliberately mutates
tile, date, RNG and settings fields to verify that mismatches fail. Artifacts and
diagnostics are retained under `.artifacts/snapshots-*`. No clock, ID or random
state fields are excluded.

## Landscape subsystem

`simulate-landscape` consumes explicit schema-version-1 subsystem JSON, not a
save file or a world snapshot. To extract a runnable fixture (using `jq`):

```sh
jq '.landscape_cases[0].before' reference/gameplay.json > /tmp/landscape.json
cargo run -- simulate-landscape /tmp/landscape.json --ticks 256 > /tmp/landscape-after.json
```

The contract includes context, raw tiles, both clocks and cached year/month values,
tile-loop cursor, gameplay RNG, and ordered per-tick events. Output can be resumed;
each request replaces the prior event trace with its newly requested ticks. Input
JSON respects `--max-bytes`; each call accepts at most 1,000,000 ticks. Paused calls
preserve all state and report empty boundary events.

Supported terrain is normal-mode temperate grass, rough, rocks and void, with
ambient NewGRF callbacks disabled. Snow, fields, water and all other tile kinds
are rejected before advancement. Void procedures can flood their neighbors in the
original game: this subsystem therefore requires all four corner heights of every
clear tile adjoining void (including diagonal neighbors and void corner heights)
to be above zero. This deliberately conservative boundary rejects potential
flooding rather than executing unported water rules. Interior sea-level terrain
away from void is allowed. The accepted terrain never changes its heights.

Clock advancement reports calendar/economy boundary order, including wallclock,
slow/frozen calendar settings, pause, tick overflow and maximum-year rewinds.
**This landscape runner does not execute boundary callback bodies.** Selected
object callbacks have a separate explicit runner described below. Terrain
uses the original LFSR schedule and grass recovery counters. Supported tile
procedures consume no gameplay RNG; the state is preserved exactly.

`check-simulation.sh` regenerates native subsystem probes, replays all clock cases
and terrain checkpoints, verifies repeatable CLI output, and checks that deliberate
tile, clock, cursor, RNG and event changes fail exact comparison. It retains native
logs and every compared JSON document under `.artifacts/simulation-*`. Ordinary
Rust tests also replay the committed independent native vectors. This proves the
stated subsystem contract, not full-game tick equivalence or save simulation.

## Selected object callbacks

`simulate-callbacks` executes one selected original callback from explicit typed
JSON state. It does not step a full world, decode object pools from a save, or
write an advanced save. For a vehicle fixture:

```sh
jq '{schema_version:1,callback:{kind:"vehicle",operation:.vehicle_cases[0].operation,state:.vehicle_cases[0].before}}' reference/callbacks.json > /tmp/callback.json
cargo run -- simulate-callbacks /tmp/callback.json > /tmp/callback-after.json
```

Implemented bodies cover vehicle calendar aging and annual profit/group accounting,
yearly house aging, yearly company expense rollover, monthly station cargo flags,
and the complete monthly industry callback in original economy with live vanilla
industries. News/AI/UI branches and other unsupported contexts are rejected.
Industry requests carry the actual callback phase before clock resets or rewinds.
See [vehicle](docs/vehicle-callbacks.md), [bookkeeping](docs/periodic-callbacks.md),
and [industry](docs/industry-callbacks.md) contracts for exact fields and limits.

`check-callbacks.sh` regenerates 34 independent native cases, compares every modeled
field through the library and CLI, and requires byte-identical repeated output.
Seven malformed/unsupported-input controls and nine deliberate output mutations
must fail. Output mutations run the real `compare` command and check the exact
path, expected value, and actual value. All compared documents and diagnostics
remain under `.artifacts/callbacks-*`; CI invokes the same driver.

## Porting direction

The target is mixed multiplayer between original OpenTTD 15.3 and Rust clients,
hosted by either implementation, with compatible saves and existing add-ons.
One deterministic Rust engine will serve desktop, browser and headless server
builds. All upstream behavior remains in the product scope.

The [compatibility roadmap](docs/roadmap.md) defines the eight stages and their
acceptance gates. The [development workflow](CONTRIBUTING.md) defines subagent
ownership, atomic commits, independent review and verification. Each stage gets
one PR; the maintainer's merge is required before work on the next stage begins.

The simulation must preserve IDs, integer behavior, random-number sequences and
consumption, update order, and command results. Rendering choices must preserve
those semantics; the preferred direction is a shared Rust renderer for native
and WebAssembly clients. See the
[verification results](docs/verification.md) and [fixture provenance](fixtures/README.md).

Licensed under GPL-2.0-only. See [COPYING.md](COPYING.md) and [NOTICE.md](NOTICE.md).
