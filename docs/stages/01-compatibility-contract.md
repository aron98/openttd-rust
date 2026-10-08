# Stage 1: compatibility contract

Status: **In progress**. Branch: `stage/01-compatibility-contract`, based on
PR #1's maintainer merge `1dcf23da8d1ceada37c0fc2d92b734011572b51e`.
The maintainer must merge this stage's PR before stage 2 begins.

## Scope and design

The versioned [contract](../../compatibility/contract.json) connects current
claims to executable drivers, their supported domains, expected observations,
input identities and retained evidence. Python 3.11+ standard-library tooling
validates this metadata and runs existing verification commands. It does not
add a production game command or replace the native comparison implementations.

Declaration and observation are different. A scenario declares `implemented`,
`reference_only` or `unimplemented`; each fresh run records `pass`, `fail` or
`not_run`. Evidence scopes distinguish Rust behavior, container preservation,
original-only behavior and future requirements. Passing an original-only probe
cannot prove Rust simulation, script execution or multiplayer interoperability.
Unimplemented requirements have no executable driver and are never run as
passing placeholder tests. Existing implemented rejection tests exercise valid
inputs outside the supported domain and assert the real application's error.

The target pins release, commit, save version and distinct protocol constants.
Game-info schema 7 is metadata, not the complete multiplayer protocol. Corpus
records include SHA-256, save version, settings profile and content identities;
NewGRFs additionally retain GRFID, MD5, parameter values and load order. Native
commands, output and packet captures make the modded reference baseline
reproducible. General NewGRF/Squirrel compatibility remains unimplemented.

## Ownership and sequence

1. A owns this plan, contract, validator/runner, its rejection tests and the
   README/roadmap/CI integration. Write negative controls before the checker.
2. B owns `scripts/native_contract*.py`, native tests and nested content/save
   fixtures. Generate a parameterized GPL NewGRF, observer script, saved world
   and original game-info captures; publish actual hashes and observables.
3. Integrate metadata and reuse the four existing native drivers unchanged.
   Run each selected driver once, retaining its exit status, logs and fresh
   artifacts. Keep the full Rust regression suite and native matrices.
4. R independently reviews boundaries, source fidelity and evidence. The lead
   verifies the integrated head and opens one stage PR. No stage 2 world model,
   gameplay loop, native multiplayer session or browser client is implemented.

## Acceptance checklist

- [ ] Release/source/save/protocol/settings/content pins are machine checked.
- [ ] Every current compatibility claim has a named scenario, supported domain,
      executable command, expected observation and evidence paths.
- [ ] Fixture and content hashes, IDs, references and status/scope distinctions
      reject meaningful mutations; malformed metadata fails before execution.
- [ ] Vanilla and modded original baselines execute from recorded recipes;
      native parameter controls and missing-content rejection are retained.
- [ ] Existing Rust tests and all four native matrices pass through the runner;
      failed commands or missing evidence cannot produce a passing report.
- [ ] Future world/gameplay/content/script, four native client/server pairings,
      mixed-client sessions and desktop/browser requirements remain explicit.
- [ ] Independent review and integrated CI pass; evidence identifies the tested
      tree and input hashes. The PR awaits the maintainer's merge.

## Verification

```sh
python3 scripts/check-contract.py --validate
python3 -m unittest discover -s scripts/tests -p 'test_*.py' -v
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
bash scripts/setup-reference.sh
REFERENCE_SOURCE="$PWD/.reference/OpenTTD" bash scripts/setup-snapshot-reference.sh
python3 scripts/check-contract.py --run baseline
```

The runner creates a new report directory for each invocation. Its report and
per-driver logs identify the exact command, exit status and child artifacts;
validation alone is never a compatibility pass. Retain artifacts in CI even
when a driver fails. Native outputs depend on the pinned build; regenerate
fixtures only through their documented recipe and review changed identities.
