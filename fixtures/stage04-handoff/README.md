# Stage 04 portable native inputs

These six cases contain unchanged **original-generated inputs**, not golden results,
portable executables, or evidence that different native binaries are equivalent.
They let a fresh clone regenerate bounded native comparisons without the historical
machine's worktrees. Only saves, actions, configuration, and this provenance travel.
The original source pin is OpenTTD 15.3 commit
`14ec60f248547d4d062a1160f0fc26d742319888`, save version 362 (see
[`upstream.toml`](../../upstream.toml)). All seven stage gates remain **OPEN**;
historical FALSE claims remain unchanged. This input addition grants no new admission.

`provenance.json` uses a finite schema: version, purpose, upstream identity,
historical binary identity, four scope/status fields, and six case records. Each case
records the call horizon, checkpoints, historical process status, driver identity,
three historical witness identities, and exactly three input identities. Each file
identity includes SHA-256, byte count, raw `stat`/`lstat` modes, and its bundle-relative
path. Historical labels are repository-relative descriptions of the original evidence,
**not paths to resolve or prerequisites to fetch**. The original `invocation.json`
`inputs[1]`, `[2]`, `[3]` supplied the save, actions, and configuration respectively;
the save is the actual native `-g` argument, not a substituted captured `initial.sav`.
Both paths can contain identical bytes. Duplicate case saves intentionally preserve
separate recipes.

| Case | State-loop calls | Checkpoints before final | Meaning on the combined branch |
| --- | ---: | --- | --- |
| forest | 60680 | prefix 8192 | Positive complete forest continuation |
| house | 60680 | prefix 6144 | Positive complete house continuation |
| timer | 60680 | prefix 1 | Positive competitor timer/random continuation |
| late-disaster | 27010 | prefix 27009 | Positive company-rating continuation |
| older-whole | 33744 | year2102 20350; age730 28490; age731 28564; quarter_prefix 33743 | Positive older-vehicle quarter continuation |
| annual-warning | 27010 | prefix 27009 | Positive whole request with unprofitable-warning news parity |

The three 60680-call test selectors retain historical rollback names. They now require
successful continuation and give **no rollback credit**. The annual-warning case now
executes the native unprofitable-vehicle warning on the final call: the regenerated
observer probes (`unprofitable_news`, `unprofitable_ai`) and the `news_queue` group
snapshot counter compare exactly against the Rust process-owned news queue; the
human-owner AI event is a native drop before storage.

## Verify the bundle

Run commands from the repository root in Bash. Git checkouts must preserve regular
non-executable files (raw mode 33188 / 0100644). Do not edit save fields, protocol
counts, config, runtime observations, or pending flags to make a comparison pass.

```bash
python3 - <<'PY'
import hashlib, json, pathlib, stat
base = pathlib.Path('fixtures/stage04-handoff')
m = json.loads((base / 'provenance.json').read_text())
assert m['schema_version'] == 1
assert m['upstream']['commit'] == '14ec60f248547d4d062a1160f0fc26d742319888'
assert m['stage_gates'] == ['OPEN'] * 7 and not m['new_native_admission']
assert not m['historical_false_claims_changed']
assert not m['historical_labels_are_dependencies']
assert {c['name'] for c in m['cases']} == {
    'forest', 'house', 'timer', 'late-disaster', 'older-whole', 'annual-warning'
}
for case in m['cases']:
    assert [f['role'] for f in case['files']] == ['input.sav', 'actions.json', 'openttd.cfg']
    for f in case['files']:
        assert f['bundle_path'] == case['name'] + '/' + f['role']
        p = base / f['bundle_path']
        assert p.stat().st_mode == p.lstat().st_mode == f['mode'] == f['lstat_mode'] == 33188
        assert stat.S_ISREG(p.stat().st_mode)
        assert len(p.read_bytes()) == f['bytes']
        assert hashlib.sha256(p.read_bytes()).hexdigest() == f['sha256']
    actions = json.loads((base / case['name'] / 'actions.json').read_text())
    assert actions['schema_version'] == 1
    count = 0
    checkpoints = {'initial': 0}
    for ordinal, action in enumerate(actions['actions']):
        assert action['ordinal'] == ordinal
        if action['op'] == 'tick':
            assert set(action) == {'ordinal', 'op', 'count'}
            assert type(action['count']) is int and action['count'] > 0
            count += action['count']
        else:
            assert action['op'] == 'checkpoint'
            assert set(action) == {'ordinal', 'op', 'label'}
            checkpoints[action['label']] = count
    checkpoints['final'] = count
    assert count == case['horizon_calls'] and checkpoints == case['checkpoints']
    save = (base / case['name'] / 'input.sav').read_bytes()
    assert int.from_bytes(save[4:6], 'big') == m['upstream']['savegame_version'] == 362
print('PASS: 18 exact inputs; six protocols and horizons')
PY
```

For full save container decoding, after the normal Rust build, run `target/debug/ottd
inspect fixtures/stage04-handoff/forest/input.sav` and repeat for the other five
cases. `inspect` validates container framing; it does not claim full gameplay parity.

## Build a fresh native observer on the remote platform

Use the current combined branch, with its tracked source manifests intact. Install
Rust, Python 3, Git and the native dependencies documented in
[the reference-cache action](../../.github/actions/reference-cache/action.yml).
For Linux that action lists CMake, g++, ccache, zlib, lzma, lzo, png and curl development
packages, xz-utils and unzip. Record the actual compiler/toolchain versions.
The commands below deliberately build from the pinned source on that machine.
The historical macOS binary hash
`1c046d71ac0f070818026fc08201dfc08e3076827d4a37a79deeaa32786f31a6`
is provenance only; do not claim a remote binary has that identity or reuse its old
admission. Record the **new** binary hash and native argv/environment.

```bash
set -euo pipefail
repo_root="$PWD"
python3 -m scripts.vehicle_year_sources "$repo_root"
python3 -m scripts.company_rating_sources "$repo_root"
bash scripts/setup-reference.sh
REFERENCE_SOURCE="$repo_root/.reference/OpenTTD" bash scripts/setup-snapshot-reference.sh

# Keep the snapshot preparation intact; extend a dedicated copy.
test ! -e .reference/stage04-company-source
test ! -e .reference/stage04-company-build
cp -R .reference/snapshot-source .reference/stage04-company-source
bash scripts/setup-company-rating-reference.sh "$repo_root/.reference/stage04-company-source"
cmake -S .reference/stage04-company-source -B .reference/stage04-company-build \
  -DOPTION_DEDICATED=ON -DOPTION_USE_ASSERTS=ON -DCMAKE_BUILD_TYPE=Release \
  "-DCMAKE_C_COMPILER_LAUNCHER=${REFERENCE_COMPILER_LAUNCHER:-}" \
  "-DCMAKE_CXX_COMPILER_LAUNCHER=${REFERENCE_COMPILER_LAUNCHER:-}" \
  "-DCMAKE_DISABLE_PRECOMPILE_HEADERS=${REFERENCE_DISABLE_PCH:-OFF}"
cmake --build .reference/stage04-company-build --parallel "${JOBS:-4}"
mkdir -p .reference/stage04-company-build/lang .reference/stage04-company-build/baseset
cp -R .reference/snapshot-build/lang/. .reference/stage04-company-build/lang/
cp -R .reference/snapshot-build/baseset/. .reference/stage04-company-build/baseset/
new_native="$repo_root/.reference/stage04-company-build/openttd"
cmake -E sha256sum "$new_native"
"$new_native" -h
```

The company setup script chains `setup-vehicle-year-reference.sh`, which chains
`setup-cargo-observer.sh`; all receive the prepared source directory, and none of
these three helper scripts builds an executable. Retain `lang` and `baseset` beside
the newly built binary. The source guards verify the tracked current branch source
closure. This bundle does not replace or modify those manifests.

## Recapture the actual native channels

Choose a new run directory. The runner requires each native output directory to be
**absent**, creates it itself, and records native input hashes, raw modes, argv,
environment, process exit status, stdout and stderr. Each command below uses the
bundle save directly as the actual native save argument. These are fresh native
runs, not imported historical outputs.

```bash
run_root="$repo_root/.omo/evidence/stage04-remote-run-01"
test ! -e "$run_root"
mkdir -p "$run_root"
for case_name in forest house timer late-disaster older-whole annual-warning; do
  fixture="$repo_root/fixtures/stage04-handoff/$case_name"
  python3 -m scripts.company_rating_run "$new_native" \
    "$fixture/input.sav" "$fixture/actions.json" "$fixture/openttd.cfg" \
    "$run_root/$case_name-native" on
done
python3 - "$run_root" <<'PY'
import json, pathlib, sys
root = pathlib.Path(sys.argv[1])
m = json.loads(pathlib.Path('fixtures/stage04-handoff/provenance.json').read_text())
for case in m['cases']:
    native = root / (case['name'] + '-native')
    assert json.loads((native / 'process.json').read_text())['returncode'] == 0
    result = json.loads((native / 'results.json').read_text())
    start = result['checkpoints'][0]['runtime']['tick']
    assert {c['label']: c['runtime']['tick'] - start for c in result['checkpoints']} == case['checkpoints']
    for label in case['checkpoints']:
        for suffix in ('sav', 'world.json', 'derived.json', 'runtime.json',
                       'movement.json', 'groups.json', 'rating.json'):
            assert (native / (label + '.' + suffix)).stat().st_size > 0
print('PASS: native exit status, horizons and required channels')
PY
```

In particular, forest/house/timer each require all **21 actual files**: initial,
prefix and final × save, World, derived, runtime, movement, groups and rating.
Retain additional schema snapshots, result/protocol records and annual/rating traces
emitted by the observer. Never manufacture missing channels from Rust output.

## Run the six exact Rust scenarios

These commands target the combined branch containing the disabled-disaster
continuation change. Each invocation must report **exactly one passed test**, zero
failures and zero ignored tests; a successful command with zero selected tests is a
failure. The function below captures the full log and enforces that count. Each
scenario gets a distinct, absent Rust output directory. No polling, retries,
prejoins, extra state-loop calls, or changed pending state are permitted.

```bash
check_one() {
  local log="$1"
  shift
  "$@" >"$log" 2>&1 || { cat "$log"; return 1; }
  cat "$log"
  python3 - "$log" <<'PY'
import pathlib, re, sys
text = pathlib.Path(sys.argv[1]).read_text()
rows = re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;', text)
assert rows == [('1', '0', '0')], rows
PY
}
for case_name in forest house timer late-disaster older-whole annual-warning; do
  test ! -e "$run_root/$case_name-rust"
done

check_one "$run_root/forest-test.log" env \
  OTTD_INDUSTRY_ROLLBACK_SAVE="$repo_root/fixtures/stage04-handoff/forest/input.sav" \
  INDUSTRY_RECIPIENT_NATIVE="$run_root/forest-native" DISASTER_OUTPUT="$run_root/forest-rust" \
  cargo test --locked -p ottd-sim --test native_industry_controls \
  natural_late_disaster_rolls_back_complete_forest_runtime -- --ignored --exact --nocapture

check_one "$run_root/house-test.log" env \
  OTTD_HOUSE_ROLLBACK_SAVE="$repo_root/fixtures/stage04-handoff/house/input.sav" \
  INDUSTRY_RECIPIENT_NATIVE="$run_root/house-native" DISASTER_OUTPUT="$run_root/house-rust" \
  cargo test --locked -p ottd-sim --test native_house_runtime_controls \
  late_native_valid_disaster_boundary_preserves_complete_house_runtime -- --ignored --exact --nocapture

check_one "$run_root/timer-test.log" env \
  OTTD_COMPETITOR_NATIVE_DIR="$run_root/timer-native" \
  INDUSTRY_RECIPIENT_NATIVE="$run_root/timer-native" DISASTER_OUTPUT="$run_root/timer-rust" \
  cargo test --locked -p ottd-sim --test native_competitor_timer \
  late_disaster_boundary_rolls_back_prior_script_random_reset -- --ignored --exact --nocapture

check_one "$run_root/late-disaster-test.log" env \
  INDUSTRY_RECIPIENT_NATIVE="$run_root/late-disaster-native" \
  COMPANY_RATING_OUTPUT="$run_root/late-disaster-rust" COMPANY_RATING_CALLS=27010 \
  cargo test --locked -p ottd-sim --test native_company_rating \
  original_quarter_matches_complete_world_and_raw_rating_cache -- --ignored --exact --nocapture

check_one "$run_root/older-whole-test.log" env \
  INDUSTRY_RECIPIENT_NATIVE="$run_root/older-whole-native" \
  COMPANY_RATING_OUTPUT="$run_root/older-whole-rust" COMPANY_RATING_CALLS=33744 \
  cargo test --locked -p ottd-sim --test native_company_rating \
  original_quarter_matches_complete_world_and_raw_rating_cache -- --ignored --exact --nocapture

check_one "$run_root/annual-warning-test.log" env \
  INDUSTRY_RECIPIENT_NATIVE="$run_root/annual-warning-native" \
  VEHICLE_YEAR_OUTPUT="$run_root/annual-warning-rust" VEHICLE_YEAR_CALLS=27010 \
  cargo test --locked -p ottd-sim --test native_vehicle_year \
  original_unprofitable_warning_matches_native_news_parity -- --ignored --exact --nocapture
```

Timer deliberately loads the **new native capture's** `initial.sav`; the native
capture itself still starts from the unchanged bundle `input.sav`. Check emitted
`PASS.json` for forest/house/timer (`calls:60680`, `rollback_credit:false`).
Company cases and annual-warning emit `result.json` with successful full calls plus
complete comparison artifacts; annual-warning additionally proves news-queue parity
against the probed native warning branch. Retain those outputs and all test logs even
on failure.

Native worker scheduling can produce a pending-state timing difference, including
`false` where a historical run observed `true`. Preserve the actual result and report
the comparison failure; do not normalize it, wait/poll until it changes, retry to
obtain a preferred outcome, or change source/input state. This recipe was checked
against tracked helper and test source; no new native build or remote-platform
capture was performed while assembling the bundle. Fresh-platform success and new
admission must be established by the actual new captures and exact test runs.
