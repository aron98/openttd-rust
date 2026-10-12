# Stage 4 remote development handoff

Use the following instructions as the next agent's task prompt. The stage source, atomic history and small original-generated input fixtures are in the GitHub branch. A fresh clone does not require the previous developer's home directory, worktrees, agent threads or large evidence folder.

## Task and authorization

Continue stage 4 as integration lead for `aron98/openttd-rust`, branch `stage/04-gameplay-parity`, draft [PR #5](https://github.com/aron98/openttd-rust/pull/5). The user authorized implementation, parallel subagents, independent reviews, atomic commits and pushing progress into this existing PR. Analyze each slice's dependencies and parallel ownership, implement it, test against the original, review and integrate it. Do not create a new stage PR, merge PR #5 yourself or start stage 5. The user reviews and merges the completed stage.

Target OpenTTD **15.3**, original commit `14ec60f248547d4d062a1160f0fc26d742319888`, save version **362**. Preserve defined original behavior, reciprocal original/Rust saves, unmodified add-ons and scripts, and the simulation prerequisites for eventual mixed original/Rust multiplayer with either server implementation. UI and playable networking remain later stages.

Read `docs/roadmap.md`, `docs/stages/04-gameplay-parity.md`, `compatibility/contract.json`, this document, and `fixtures/stage04-handoff/README.md` plus its provenance manifest. The previous machine's ignored evidence and team state are historical records, not prerequisites for this checkout. Initialize new agents/worktrees and local evidence on this machine using available tooling.

All seven broad stage gates remain **OPEN**:

1. Content-dependent runtime restoration and invalidation, vanilla and modded.
2. Construction, terrain and water.
3. All vehicle families, pathfinding, service and lifecycle.
4. Cargo and station simulation.
5. Towns, industries, economy and company lifecycle.
6. NewGRF execution and persistent content state.
7. Complete compatible Squirrel/AI/GameScript execution and host APIs.

The prior estimate was **75–85% remaining** by behavior coverage. This is a qualitative estimate, not an audited percentage or a reason to narrow the roadmap.

## Source already preserved

The branch includes the previously published company/annual/script integration and the two formerly local increments. Their atomic implementation, test and binding commits remain in nonsquash history.

- Company source composition: `df371959d778c23b43febdf66e78a7ccd5f43ff2`, following annual vehicle/group-profit and script reference ownership work. Quarterly scoring owns all150 raw score slots and preserves native history/arithmetic order and transaction publication.
- Disabled-disaster component: `3802d5f864316c3dfebbce16c713a23facc6f578`. Product `2c56239489d8714c75d47316008220e24c0c6a13`, continuation tests `2167031de77f78f3fab1c6518564a8218a6b3efb`, then separate bindings. Unsigned delay decrement/reset draws once through the shared RNG before industry work. Enabled disasters still refuse.
- Cumulative script metadata component: `4fc0c9155af866cc44b842bbbd5b39ca89f47a09`, from reviewed `ef8b91669008a42ba0ddd7236a8abf30fa71b7c3`. Includes mechanical RootSlot extraction `c301deec77d72fd8bb6253b87081d44e1500489f`, passive native observations `4676cfefa84d7afc28b45db35269b160f10db49f`, actual heap/runner graph controls `3072e1f058d71507ee3e811be173789d3aadc0ee`, session ownership design `89fe5c60233bc64658d33809dfef5ff0ee3c95e0`, native registration metadata `97370f37e72e35e925ccc08c7298c00730f84372`, formatting and bindings.
- The exact334-entry worlds artifact list now uses the existing SHA-pinned external-list mechanism. Expanded driver requirements are unchanged. No parser/schema extension or cap increase was made.
- The portable fixture bundle contains original save arguments, action protocols and configs for forest, houses, timer, late-disaster, older-whole and annual-warning captures. These inputs are not golden output or a portable original executable.

Native registration metadata preserves70 exact source rows and44 callback identities. Existing four base functions execute;66 default registration rows are metadata only. `NativeFunction::Default(DefaultFunction)` is an intentional public enum expansion. Known default calls produce an explicit uncatchable unsupported-function error without fabricating native arity diagnostics or changing prior error/argument effects. Read-only descriptors do not install tables or create a full VM.

Full shared VM, physical RefTable, native default bodies, materialized NativeSession and AI/GameScript host are still missing. Four genuine shared-table/friend graph comparisons fail in debug/release. Their authentic failures are required open evidence, not tests to weaken, delete or turn into synthetic passes. Read the committed ownership/session design before proposing the next bounded implementation slice.

## Fresh-machine setup and verification

If the repository is not already present:

```sh
git clone https://github.com/aron98/openttd-rust.git
cd openttd-rust
git switch stage/04-gameplay-parity
```

Otherwise inspect existing work before fetching/switching. Verify the branch and latest PR head, and confirm both component commits above are ancestors. Do not reset unrelated work or force-push.

Use a current stable Rust toolchain compatible with the repository, Python3, `uv`, CMake and a C++ compiler. Consult `.github/workflows/ci.yml` and `.github/actions/reference-cache/action.yml` for the actual CI environment and native dependencies. The Linux native dependencies include cmake, g++, ccache, zlib, lzma, lzo, png, curl development libraries, xz and unzip. Do not copy macOS executables or SDK paths to Linux.

Run ordinary checks on this checkout:

```sh
python3 scripts/check-contract.py --validate
python3 -m unittest discover -s scripts/tests -p 'test_*.py' -v
PYTHONPATH=. uv run --with pytest python -m pytest scripts -q
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked
```

Ordinary suites contain ignored original-backed consumers. A green ordinary suite is not proof that those compatibility scenarios executed. Inspect driver declarations and run the relevant actual drivers with nonzero named test counts.

Prepare the pinned original with the tracked scripts. For ccache-enabled native builds, use the same launcher/PCH settings as CI and record actual compiler/toolchain identities. A reasonable bounded starting point is:

```sh
export REFERENCE_COMPILER_LAUNCHER=ccache
export REFERENCE_DISABLE_PCH=ON
export CCACHE_DIR="$PWD/.reference/ccache"
export JOBS=4
bash scripts/setup-reference.sh
bash scripts/setup-snapshot-reference.sh
python3 scripts/check-contract.py --run baseline
```

First check driver disk requirements and available space. Baseline execution is substantial; run it once per relevant source epoch, use compiler caching, retain real failures and let an active run finish. Do not cancel/restart CI or add retries to obtain a favorable result. The fixture README explains the separate company/annual observer preparation and the exact environment/consumer mapping for regenerating the new long continuations and genuine rollback test. Do not overwrite a shared original build while another worker is using it.

The script normal producer and table driver regenerate their own compiled bundled-Squirrel observations from committed sources. `scripts/compat/script-vm/tables/native_roots.py` captures the finite native shared/context sessions. Follow the actual committed producer entrypoints and flags. Rebuild and recapture on the new machine; do not rewrite historical absolute argv, executable hashes or source locations to make moved archives pass provenance checks.

Historical local approvals/check counts are records of that machine's source epochs. New binaries, platform behavior, compiler outputs and observations require new identities. A macOS binary hash is not a required Linux binary hash. If a remote comparison fails, retain its raw outputs and investigate the difference; do not use prior success to waive it.

## Meaningful current compatibility seams

The timer change originally removed actual refusal at disaster expiry. Local original-backed positives covered27010 and uninterrupted33744 road calls plus forest/house/timer60680. Their independent prefixes are8192/6144/1 respectively. Historical rollback-named60680 selectors now prove positive continuation and earn **no rollback credit**.

The annual-warning fixture has a genuine27009-call prefix before an unsupported warning event. The original advances the final call; Rust's whole27010 request must refuse exactly:

```text
unsupported world tick phase vehicle_year: call 27009: unprofitable warning/news and AI event context
```

The test checks complete committed World/derived/cache/transport equality and the actual original threaded worker Arc identity, count and backend. Compare all150 raw company ratings during the successful prefix. This proves rollback, not implementation of news/AI behavior or equality with the original's final warning state. Require the exact phase/error string beyond the existing generic harness's substring assertion.

Native road pool fields and per-company unit-number `next` values are credited by `580eb6a` whole-runtime comparisons; the native units `count`, other group fields and wider ownership/HQ/news/AI contexts remain uncredited. Stopped rail restoration is not train movement. Metadata registration is not default-method execution.

Keep previous failed forest/async/route/RNG epochs explicit, including forest36/38 cuts and5/6 origin continuations,512/6400/native3081 failures, raw561/scale/expanded1247 transport epochs, paused-load initial movement and unsaved interactive RNG. New bounded passes do not erase unrelated failures or prove universal tracing neutrality.

## Next implementation and working discipline

After establishing this machine's baseline, divide independent source-grounded slices among workers. The lead owns interfaces, shared observer/build scheduling, integration and final QA. Give workers explicit file ownership and isolated worktrees; tell them they are not alone and must preserve others' edits. Use one fresh independent leaf reviewer per frozen component/final integration, not a panel or a retasked terminal reviewer.

Promising script work is actual materialized shared-state/session construction following the committed design: native constructor order, genuine default tables, root/stack/registry ownership, physical RefTable and callback state. Metadata does not license fabricating graph nodes. Establish a bounded authentic original RED before implementing a factory. In parallel, choose startable transport/world/content slices from the full stage plan: broader road/service/events, rail reservations/movement, ships/aircraft, town/industry lifecycles, construction/water and NewGRF callback consumers. Analyze shared dependencies first; do not just spawn vague owners.

Keep production runtime in Rust. Observe the original only as an independent oracle. Preserve exact ordering, RNG, identity, allocation and transaction semantics. Never add polling, waits, prejoins, favorable retries, field projections, normalization or hash relabeling to obtain parity. Do not execute source-proven original UB, malformed/crashing VM/lexer/badtail or sanitizer probes. The user approved explicit Rust rejection of undefined native inputs; valid Unicode strings/comments must remain covered.

Keep changed Rust/Python modules within the existing250 pure-line policy, including attributes, with no lint/size exemptions. Preserve strict guards, typed errors, exact source/copy memberships, all143 scenario policies and36 expanded drivers. The contract cap is still1MiB; use the existing bounded literal-layout mechanism when necessary, never delete requirements or raise limits to hide growth.

Run appropriate native and regression tests, record actual compiler-selected executable/source/dependency provenance and meaningful corruption/refusal controls, perform root manual QA, obtain independent review, then integrate and push atomic progress into PR #5. Reuse immutable proof in place; avoid recreating the old enormous folder by nesting archives, retaining unlimited target caches or repeatedly copying native toolchain closures. Delete only proved inactive rebuildable caches, never unique input/output proof or selected binaries required by current checks.

CI must remain PR-only with native compiler caching. Check the latest PR jobs before claiming success, never cancel a live run, and do not represent pending CI as passed. Keep PR #5 draft until every stage requirement and required verification is actually complete. Only the user's merge authorizes stage5.
