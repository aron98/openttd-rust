# Stage 1: compatibility contract

Status: **Completed**. Branch: `stage/01-compatibility-contract`, based on
PR #1's maintainer merge `1dcf23da8d1ceada37c0fc2d92b734011572b51e`.
[PR #2](https://github.com/aron98/openttd-rust/pull/2) was merged by the maintainer
as `e35f0f7375d84124f8047adff404eb4273b7277b`; stage 2 starts from that merge.

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

- [x] Release/source/save/protocol/settings/content pins are machine checked.
- [x] Every current compatibility claim has a named scenario, supported domain,
      executable command, expected observation and evidence paths.
- [x] Fixture and content hashes, IDs, references and status/scope distinctions
      reject meaningful mutations; malformed metadata fails before execution.
- [x] Vanilla and modded original baselines execute from recorded recipes;
      native parameter controls and missing-content rejection are retained.
- [x] Existing Rust tests and all four native matrices pass through the runner;
      failed commands or missing evidence cannot produce a passing report.
- [x] Future world/gameplay/content/script, four native client/server pairings,
      mixed-client sessions and desktop/browser requirements remain explicit.
- [x] Independent review and integrated CI pass; evidence identifies the tested
      tree and input hashes. The maintainer merged PR #2.

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

## Handoff evidence

The reviewed implementation is
`17e5641baec6041fa1c7957a7fc656d084cb13d2`. Independent review approved this
commit with no blocking findings. Integrated local QA and
[CI run 37837626494](https://github.com/aron98/openttd-rust/actions/runs/37837626494)
passed for that branch head; both the Rust and interoperability jobs succeeded.
CI checked out GitHub's synthetic PR merge
`69aee7612c9cfebe4c017405834751ef6b316b77`, while local QA and independent
review checked the implementation commit above. This readiness update changes
documentation only.

The contract has 83 declarations. All six baseline drivers passed: 64 scenarios
passed (61 implemented and **three original-only reference scenarios**), while
all 19 future requirements remained `not_run`. Verification includes 13 Python
test methods with rejection subtests, 94 Rust tests, the four native comparison
matrices and the original-only content/protocol baseline. External-oracle tests
ignored by plain workspace testing were exercised by their native drivers.
Formatting and strict Clippy also passed.

The [retained CI artifact](https://github.com/aron98/openttd-rust/actions/runs/37837626494/artifacts/11576910894),
`compatibility-contract-37837626494-1`, contains the fresh contract report,
per-driver logs and native captures. The report identifies the manifest hash,
input hashes, commands, exit statuses and concrete evidence paths. Reproduce it
with the verification commands above; validation alone is not a baseline run.

The original-only scenarios establish content effects, a recorded native
command and prejoin game-info metadata. They do not demonstrate Rust NewGRF/AI
execution, command replay, multiplayer joining or synchronized sessions.
The maintainer merge completes this stage; stage 2 is tracked in its own plan.
