# Compatibility contract

[`contract.json`](contract.json) is the versioned inventory of current claims
and future requirements. It targets the exact release/commit/save version in
[`upstream.toml`](../upstream.toml). The checker uses Python 3.11+ standard-library
test tooling; the game remains Rust.

```sh
python3 scripts/check-contract.py --validate
python3 scripts/check-contract.py --list
python3 -m unittest discover -s scripts/tests -p 'test_*.py' -v
bash scripts/setup-reference.sh
REFERENCE_SOURCE="$PWD/.reference/OpenTTD" bash scripts/setup-snapshot-reference.sh
python3 scripts/check-contract.py --run baseline
# Or one driver/scenario, still exercising that driver's complete matrix:
python3 scripts/check-contract.py --run callbacks
python3 scripts/check-contract.py --run reference.game-info
```

`--validate` checks metadata only and says that no scenarios executed. It checks
strict fields, IDs/references, pinned target values, fixture/save header hashes,
content MD5/parameter ranges and exact native content profile, local source
references and status/evidence scope. It checks protocol constants against the
original header when the reference checkout exists (native CI), without requiring
that checkout for offline validation.
The manifest is limited to 1 MiB. The declared upstream protocol constants are
for the pinned source: game-info 7, admin 3, coordinator 6 and survey 2. Game-info
is a metadata schema, not a single version number for the whole game protocol.
The native baseline exercises actual query/response packets 7/6 and release
revision `15.3`; it does not join or play a session.

## Declarations and observations

| Declaration | Evidence scope | Meaning |
| --- | --- | --- |
| `implemented` | `rust_behavior` | The bounded Rust behavior has executable checks. |
| `implemented` | `container_preservation` | Rust preserves container bytes; native alone continues the saved world. |
| `reference_only` | `original_only` | The original establishes a future porting baseline. |
| `unimplemented` | `future` | A required future capability with no passing placeholder command. |

Fresh reports separately record `pass`, `fail` or `not_run`. A declaration is
never a test result. Selecting an unimplemented requirement returns an explicit
nonzero checker diagnostic; this is inventory handling, not proof that a game
implementation rejects a real request. Actual supported-domain rejection claims
are implemented test scenarios exercising the Rust CLI or native loader with
otherwise valid fixtures.

A selected scenario runs its entire shared driver once. `baseline` executes
workspace tests, container rewriting, typed snapshots/primitives, clock/terrain,
selected callbacks and the original-only content/protocol suite. The existing
four shell comparison drivers remain authoritative; ignored external tests are
executed by those drivers, not counted from a plain `cargo test` skip. Setup
builds are explicit prerequisites. No source checkout, comparison or test result
is cached by this runner.

## Corpus and evidence

Assets carry paths, SHA-256 and provenance. Saves additionally declare their
header version and profile; profiles link hashed settings sources (config,
recipe and/or authoritative save bytes). Settings prose is descriptive, not a
second machine-verified decoding of the saved world. Profiles retain ordered NewGRF
identities, MD5 and parameters. The three original fixtures remain the existing
container/snapshot corpus. The nested [modded fixture](../fixtures/contracts/README.md)
is original-only, with a self-authored parameterized GRF and native observer AI.
It demonstrates a real speed effect, AI save/reload, recorded company rename and
prejoin metadata. One GRF does not establish multi-GRF load-order semantics;
opaque save preservation does not implement NewGRF or Squirrel execution.

Each invocation creates `.omo/evidence/contract-*` (or an absent `--run-dir`).
`report.json` records the manifest path/hash, tested commit, working-tree status,
input hashes and each scenario's declaration/scope/observation. Per-driver
`stdout.log`, `stderr.log` and `result.json` retain command, deadline result and
artifact location. Child directories must be newly created under the declared
root; required files must be nonempty and resolve inside that directory. Failure,
timeout, missing output and stale evidence cannot yield a passing report. Driver
timeouts kill the process group on the supported Linux/macOS test hosts.

The PR workflow uploads these narrowly selected hidden report/native artifact
directories on success or failure. A local absolute path in a receipt refers to
the corresponding directory basename in the uploaded tree. It uploads neither
compiler cache/source checkouts nor unrelated agent working notes. A fresh native
failure remains a failure even if a checked-in golden output looks correct.

For review, change a claim and its scenario together. Regenerating an asset
changes its hash and requires reviewing its recipe and native result; never
update a golden merely to silence a mismatch. Required future world, gameplay,
content/scripts, native mixed sessions, desktop and browser gates are listed
explicitly and remain unimplemented until their own stages establish evidence.

Protocol attribution: the pinned upstream
[network constants](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/network/core/config.h),
[TCP packet enumeration](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/network/core/tcp_game.h)
and [game-info encoding](https://github.com/OpenTTD/OpenTTD/blob/14ec60f248547d4d062a1160f0fc26d742319888/src/network/core/network_game_info.cpp)
provide the baseline. The query proves those observed metadata fields only;
command negotiation, synchronization and playable mixed sessions remain future
requirements.
