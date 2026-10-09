# OpenTTD Rust

A Rust port targeting **OpenTTD 15.3**, with behavioral parity and two-way save
compatibility as the end goal. The exact upstream commit is recorded in
[`upstream.toml`](upstream.toml).

**Current state: saved-world tools, supported command replay, bounded world ticks,
typed snapshots, and selected object callback bodies. This is not yet a playable game.**
The Rust code reads and rewrites save containers while preserving chunk contents.
Version-362 saves additionally decode into typed map, clock, settings and saved
random-state snapshots. The `world` tools expose pinned saved tables and maps,
validate object references, and restore content-independent structural indexes.
Content-dependent gameplay caches remain a stage 4 requirement. Rust randomizer, map-coordinate and Gregorian calendar
primitives are checked against independently generated C++ vectors.
An explicit subsystem fixture can advance calendar/economy clocks and temperate
clear terrain against original C++ results. A separate typed-state runner executes
selected vehicle, house, company, station, and industry callbacks. Saved-world replay
integrates clocks, terrain and admitted company/town callbacks with command
validation, costs and execution. General gameplay, rendering,
scripts and multiplayer are not implemented.

## Use

```sh
cargo run -- inspect fixtures/generated-v362.sav
cargo run -- snapshot fixtures/generated-v362.sav > /tmp/world.json
cargo run -- world fixtures/generated-v362.sav --view saved > /tmp/saved-world.json
cargo run -- world fixtures/generated-v362.sav --view derived > /tmp/structural-state.json
cargo run -- edit-world fixtures/generated-v362.sav edits.json /tmp/edited-world.sav
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

`world` loads version 362 against the pinned schema manifest. Its default JSON
envelope contains `schema_version`, `saved` and `derived`; `--view saved` and
`--view derived` export those trees separately for exact comparison. Saved strings
are byte arrays and pointer fields retain native `id + 1` encoding, with zero
meaning null. Ordinary ID fields use their native sentinels. The derived tree
contains structural links and aggregates, not content-dependent gameplay caches.

`edit-world` applies a strict JSON edit document, validates the resulting world
as one transaction, then publishes a new native save. For example, `edits.json`
can contain:

```json
{"schema_version":1,"edits":[{"kind":"field","chunk":"PATS","record":0,"path":["difficulty.max_no_competitors"],"value":{"unsigned":3}}]}
```

Field paths alternate exact field names and list indices; dots inside a field
name are literal. Values have explicit `signed`, `unsigned`, `bytes` or `array`
tags, with tagged elements inside arrays. Existing nested leaves can be edited;
struct-list replacement and primitive-array resizing are unsupported. A tile
edit uses `{"kind":"tile","index":N,"value":TILE}` with all raw tile fields
from `snapshot`: `type`, `height`, and `m1` through `m8`. This is saved-state
editing, not construction or gameplay-command validation.

Edit files reject duplicate/unknown keys and inexact numbers. They are limited
to 1 MiB, 1024 operations and 64 path elements; a smaller `--max-bytes` also limits
the document. No destination is created if any edit fails, and an existing
destination is never overwritten. The default compression matches the source;
`--compression` accepts the same formats as `rewrite`.

The default encoded/decompressed byte limit is 256 MiB; change it with
`--max-bytes`. This is not a total process-memory cap. The XZ decoder separately
limits its memory to 256 MiB. There is a 4096-chunk structural limit.

The header reader accepts versions 1 through 362 except upstream's reserved
patchpack range 220 through 286. Real interoperability has been checked for the
three fixtures at versions 211, 308, and 362. Version-zero/headerless saves,
original TTD saves, patchpacks, future save versions, and historical semantic
migrations are not implemented by Rust yet. A rewrite never upgrades a header
without migrating its state.

## Saved-world replay

`replay-world` executes an ordered versioned action file against a version-362
save. `resume-world` continues a checkpoint's pending actions in a fresh process.
These examples use the committed [replay corpus](fixtures/replay/README.md):

```sh
cargo run -- replay-world fixtures/replay/clear-v362.sav fixtures/replay/ticks.json /tmp/ottd-prefix --through 3
cargo run -- resume-world /tmp/ottd-prefix/checkpoint.json /tmp/ottd-suffix
cargo run -- replay-world fixtures/replay/populated-v362.sav fixtures/replay/populated-road.json /tmp/ottd-road
```

Choose output directories that do not already exist. Each result contains action
receipts, named native-compatible saves, complete saved and structural JSON,
deterministic runtime observations, and `checkpoint.json`. The envelope pins the
native revision, save version and sibling `final.sav` SHA-256, and retains the
action cursor and pending actions. It is separate from the native save format.
`--through` is an inclusive action ordinal, independent of the saved simulation
tick. Commands preserve FIFO order while paused. Checkpoint labels are unique
ignoring ASCII case, including the reserved `initial` and `final` labels.

Supported commands are flat vanilla road construction, supported clear-ground
clearing, loans, company/president names, and headless pause transitions. Receipts
retain native test, affordability and execution behavior, including estimates
and early failures. Populated command fixtures stay paused for Rust state-loop
calls. Unpaused ticks admit temperate clear/void worlds with dormant towns, human
companies, no unsupported active pools, and settings/counters that keep events
inside the implemented domain. Company finance, expense/history rollover, town
ratings/history, clocks, tile scheduling, global counters and paused construction
limit refill execute. See the [stage 3 domain](docs/stages/03-commands-tick-execution.md)
for exact admission and rejection rules.

Load admission rejects missing companies/towns, active scripts/NewGRFs and
unsupported linkgraph load state. A later unsupported event rejects the whole
replay before publication. The pure Rust replay kernel performs no filesystem
I/O; the CLI adds checkpoint SHA-256 verification using `sha2` and stages output
privately. Publication reserves a new directory and never overwrites an existing
one. A filesystem failure during final publication can leave a partial directory
of validated artifacts; `results.json` is published last and the error reports
that limitation. Plans are limited to 10,000 actions and 100,000 cumulative ticks;
the CLI bounds action/envelope JSON at 16 MiB and total output at 512 MiB.

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
python3 scripts/check-contract.py --run worlds
python3 scripts/check-contract.py --run commands.world-ticks
```

See the [contract guide](compatibility/README.md) and
[stage 1 plan](docs/stages/01-compatibility-contract.md). Native NewGRF/AI and
prejoin protocol probes are reference baselines; they do not establish Rust mod
execution or multiplayer support. The world driver covers five native fixtures,
exact saved/structural comparisons, changed-state reloads in the instrumented and
unmodified original engines, and deliberate comparison/cache failures. Its
artifacts are retained with the contract report in PR CI. Content-dependent
runtime restoration remains an explicit stage 4 gate.
The replay driver compares complete checkpoints through the public CLI and the
unchanged native state loop, including fresh-process resume, numeric overflow
compatibility and original continuation of Rust road construction. It does not
claim general gameplay, content runtime or multiplayer parity.

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
bash scripts/check-replays.sh
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
