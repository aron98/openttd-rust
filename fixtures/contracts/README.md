# Original-only Stage 1 baselines

These files establish narrowly observed behavior in **unmodified OpenTTD 15.3**,
source commit `14ec60f248547d4d062a1160f0fc26d742319888`, save version 362.
They do not establish Rust NewGRF, AI, full-world loading, gameplay command
execution, network joining, or multiplayer compatibility.

## Content and persisted fixture

`modded-v362.sav` is a 64×64 temperate world generated with seed 12345, starting
year 1950, OpenGFX 7.1, and 500 null-video-driver ticks. It uses precisely one
selected NewGRF, `contract-speed.grf`, with parameter `[123]`, and one native-only
observer AI, `ContractProbe` version 1/API 15. The complete asset hashes,
identity and expected effect are in [metadata.json](../content/metadata.json).
The save hash pins this retained input; fresh generation is **not** claimed to
produce identical bytes (session IDs and other runtime details can differ).

The self-authored GPL-2.0-only GRF uses Action 8 for identity, Action 6 to copy the
low two bytes of parameter 0 into its next pseudo-sprite, and Action 0 to set
train engine 0's property `0x09` (`PROP_TRAIN_SPEED`). Source is
[contract-speed.hex](../content/contract-speed.hex); its deterministic container
encoding is `grf_bytes()` in `scripts/native_contract_content.py`. No GRF compiler,
external content download, sprites, or third-party add-on is needed. The GRFID
`52555354` (ASCII `RUST`, numeric little-endian ID `0x54535552`) is a local fixture
identifier, not a claim of registration or compatibility with unrelated content.

The native observer calls `AIEngine.GetMaxSpeed(0)`. Upstream
`src/script/api/script_engine.cpp:114` forwards to `Engine::GetDisplayMaxSpeed`,
and the train branch in `src/engine.cpp:357` reads `PROP_TRAIN_SPEED` without a
unit conversion. The measured values are 123 and 77 native “km-ish/h” for
parameters 123 and 77. The harness checks both, and verifies that asserting 123
against the 77 run fails. A single selected GRF preserves a one-element load
order; this does not test interactions or ordering between multiple add-ons.

The observer also calls `AICompany.SetName("Contract Probe")`, records its true
return value and resulting name, and saves `{contract_marker = 123}`. Both a
freshly generated save and the retained fixture are reloaded by the original
engine; its logs must show speed 123, restored marker `true`, and the persisted
company name. AI behavior is observed only in the original Squirrel runtime.

## Reproduction and recorded commands

Prerequisites: Python 3.10+ (standard library only), and the pristine pinned
native build/assets from `scripts/setup-reference.sh`. Run from repository root:

```sh
python3 -m unittest discover -s scripts/tests -p test_native_contract.py
python3 scripts/native_contract.py
```

Optional `--oracle PATH` selects the native binary. `--run-dir FRESH_PATH` selects
a fresh artifact directory; existing directories fail rather than being reused.
The default prints `Artifacts: <absolute path>` before executing. Each case
retains exact executable argv in `commands.json`, configuration, stdout, stderr,
process result, native saves and (where enabled) `save/autosave/commands-out.log`.
The driver exits nonzero if any assertion fails and writes `failure.log`.

The generation recipe installs the assets under isolated `newgrf/` and `ai/`
directories and writes `scripts/game_start.scr` containing:

```text
start_ai ContractProbe
```

Then it runs (placeholders refer to that isolated case):

```text
<oracle> -X -x -c <case>/openttd.cfg -vnull:ticks=500 -snull -mnull -d sl=2,grf=1,script=2,desync=2 -g -G 12345
```

The native AI executes the company rename. [commands-out.log](commands-out.log)
is the unedited original command trace retained when creating the fixture;
initial capture used `grf=8` instead of `grf=1` for extra diagnostic verbosity.
The harness compares fresh command records, excluding only host wall-clock
prefixes, to these two exact records:

| Native command | Economy date | Fraction | Company | Command ID | Error message ID | Serialized arguments |
| --- | --- | --- | --- | --- | --- | --- |
| `CmdCompanyCtrl` | `0x000ade1f` (712223) | 0 | `0xff` | `0x5a` | 0 | `01FF0001000000` |
| `CmdRenameCompany` | `0x000ade1f` (712223) | 4 | 0 | `0x3e` | 0 | `436F6E74726163742050726F626500` |

The date field is `TimerGameEconomy::date`; it equals the calendar date in this
profile. The rename payload is the NUL-terminated UTF-8 company name. Original desync
recording comes from `CommandHelperBase::LogCommandExecution` in
`src/command.cpp` and `src/debug.cpp`'s `commands-out.log` writer. These are real
native command records and a reproducible recipe, **not** a frame-scheduled Rust
replay, nor a completed replay test of the original desync recording. The initial
AI-launch command and generated-world boundary are retained as recorded; the
harness does not rewrite their order or claim they are all post-load gameplay.

## Protocol baseline

The driver starts its own original dedicated server bound to `127.0.0.1` with
local advertising and an ephemeral port. It queries both the existing vanilla
fixture and this modded fixture, validates successful requested-save loading,
and always terminates/reaps its child. Startup, subprocesses and socket reads
have deadlines; packet size is capped at 16384 bytes.

Request bytes `03 00 07` are TCP `PACKET_CLIENT_GAME_INFO` (type 7). Response type
6 is `PACKET_SERVER_GAME_INFO`. **7 is the game-info payload schema**, not a
universal gameplay protocol version. The original default response uses
serialization mode 1: GRFID, MD5 and name. The parser validates framing, exact
schema/mode, bounded strings, boolean encoding and complete payload consumption.
The observed revision must be `15.3`, the server name must match the owned case,
map/landscape/start date must match, and selected content identity/MD5/name must
be exact (empty for vanilla). GameScripts are absent in these fixtures.

`game-info-vanilla.bin` and `game-info-modded.bin` are retained actual original
server responses used by parser tests. Fresh runs retain `query.bin`,
`response.bin`, `game-info.json` and raw wrong-revision request/response packets
inside `protocol-vanilla/` and `protocol-modded/`. A deliberately wrong revision
is rejected before authentication with bytes `04 00 03 08` (`SERVER_ERROR`,
`NETWORK_ERROR_WRONG_REVISION`). Truncated/trailing responses and schema 8 are
rejected by the harness parser. Those mutations test our boundary, not original
server handling of malformed packets. No authentication, map transfer, gameplay
commands, late joins or frame synchronization is attempted.

Pinned implementation references: `src/network/core/tcp_game.h` (packet types),
`src/network/core/network_game_info.cpp` (serialization), its header (default
`send_newgrf_names=true`), and `src/network/network_server.cpp` (query and
pre-auth revision rejection). UDP discovery in this revision replies with an
empty discovery packet; game metadata is queried over TCP.

## Negative content setup

A valid saved world and its AI are also loaded **without** the required GRF.
Original singleplayer exits zero and emits `[grf:0] NewGRF ... not found`;
it can save the incomplete paused state. The harness rejects that warning before
checking observer output, retaining `process.json` with native exit status 0,
the raw diagnostic, and `expected-rejection.log`. This is harness rejection of
an invalid reference setup, not a claim that the original engine hard-rejects
missing content. The successful setup additionally requires observer state,
exact advertised content and successful native save/load logs, so an unrelated
fallback cannot count as a valid baseline.
