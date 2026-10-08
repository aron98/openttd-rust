# OpenTTD Rust

A Rust port targeting **OpenTTD 15.3**, with behavioral parity and two-way save
compatibility as the end goal. The exact upstream commit is recorded in
[`upstream.toml`](upstream.toml).

**Current state: save compatibility foundation. This is not yet a playable game.**
The Rust code reads and rewrites save containers while preserving chunk contents.
It does not yet interpret game objects, advance a simulation, render the game,
run scripts, or connect to multiplayer servers.

## Use

```sh
cargo run -- inspect fixtures/generated-v362.sav
cargo run -- rewrite fixtures/generated-v362.sav /tmp/openttd-roundtrip.sav --compression lzma
cargo run -- --help
```

`inspect` prints JSON containing the save version, compression, and chunk framing.
`rewrite` supports `none`, `zlib`, `lzma` (XZ), and `lzo`. Omitting `--compression`
preserves the source format. The destination must not exist; writes are published
only after successful decoding and encoding. Versions and all chunk bodies are
preserved, including unknown chunks. An unknown chunk may still be rejected by
OpenTTD itself: this tool checks container structure, not gameplay validity.

The default encoded/decompressed byte limit is 256 MiB; change it with
`--max-bytes`. This is not a total process-memory cap. The XZ decoder separately
limits its memory to 256 MiB. There is a 4096-chunk structural limit.

The header reader accepts versions 1 through 362 except upstream's reserved
patchpack range 220 through 286. Real interoperability has been checked for the
three fixtures at versions 211, 308, and 362. Version-zero/headerless saves,
original TTD saves, patchpacks, future save versions, and historical semantic
migrations are not implemented by Rust yet. A rewrite never upgrades a header
without migrating its state.

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
```

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

## Porting direction

The next milestone is typed game-state decoding and historical migrations. Then
the deterministic simulation and command system can be ported against upstream
state comparisons, followed by full content, script, network, and UI parity.
All upstream behavior remains in the product scope.

The simulation must preserve IDs, integer behavior, random-number sequences and
consumption, update order, and command results. A graphics engine or ECS will not
be chosen before those semantics are mapped. See the
[verification results](docs/verification.md) and [fixture provenance](fixtures/README.md).

Licensed under GPL-2.0-only. See [COPYING.md](COPYING.md) and [NOTICE.md](NOTICE.md).
