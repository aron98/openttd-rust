# CI reference-build cache

The test workflow runs only on `pull_request`. Its interoperability job uses
ccache for both pinned OpenTTD reference builds and always runs the compatibility,
snapshot, landscape, and callback matrices. A separate merged-PR workflow warms
the shared compiler cache; it runs no Rust commands or tests. Neither workflow
has a push, schedule, or manual trigger. Cache hits never skip required builds or
reuse test results.

Only `.reference/ccache` is saved by `actions/cache`. Source checkouts, CMake build
trees, executables, graphics assets, Rust outputs, and comparison artifacts are not
cached. The compiler cache is bounded to 2 GB; its configuration and per-job hit,
miss, and uncacheable-call statistics are printed in the workflow log.

## Cache keys and validation

The primary key contains:

- Runner OS and architecture, plus the hosted image identity/version.
- GCC version and content hashes of the C++ driver and `cc1plus` backend, plus
  CMake and ccache versions.
- Hashes of `upstream.toml`, both setup scripts, reference patches/headers, and the
  two workflows and the shared `.github/actions/reference-cache` action.

A fallback restore key retains the same runner/toolchain fingerprint but allows
reference inputs to differ. This permits reuse of unaffected compilations after
an instrumentation change. ccache still checks compiler contents, compilation
options, source, and included headers before returning an object; no sloppiness
options are enabled. A fallback archive is a candidate cache, not evidence that
any particular compilation is reusable. See the [ccache manual](https://ccache.dev/manual/latest.html)
and [GitHub cache-key behavior](https://github.com/actions/cache#usage).

`CCACHE_BASEDIR` is the workspace root so paths within the checkout can be
normalized. This does not promise sharing across different operating systems,
architectures, toolchains, or arbitrary directory layouts. The conservative
fingerprint deliberately starts a new cache when the hosted image/toolchain changes.

The snapshot build clones from the original checkout already verified earlier in
the same job through `REFERENCE_SOURCE`. It still performs its own pinned-commit,
patch, and tracked-source verification. Graphics archive hashes remain checked.
Neither source verification nor any comparison matrix has been weakened.

## Shared cache after a merge

`cache-warmer.yml` handles only closed PRs merged into this repository's default
`main` branch. Its actual job condition checks the merged flag, branch, default
branch, event activity, and base/workflow repository identities. Before checkout,
a shell guard requires a 40-digit hexadecimal merge commit SHA. Checkout uses
only that immutable `merge_commit_sha`, never a PR head or `refs/pull` reference,
and sets `persist-credentials: false`. Repository permissions remain
`contents: read`; no additional secrets are passed.

The warmer explicitly sets job `cache-mode: write`, which is needed for
`pull_request_target` to save in the default-branch cache scope. The common action
uses `actions/cache@v4`, including its successful-job post-save. PR jobs retain
normal merge-ref-scoped cache saves for their own iterations; those caches are
not promoted into `main`. Future PRs can also restore the cache created in the
base/default branch. See [GitHub's cache access and scope rules](https://docs.github.com/en/actions/reference/workflows-and-actions/dependency-caching)
and [job cache-mode syntax](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax#jobsjob_idcache-mode).

Both jobs call the same local composite action. Dependencies, compiler/cache
settings, fingerprint, full primary key, and fallback prefix therefore have one
definition. The `cpp-v1` fingerprint algorithm and complete runner/toolchain
fallback prefix are unchanged, allowing an existing same-PR cache to remain a
fallback when the reference-input hash changes. Both setup scripts still verify
the pinned source and instrumentation before configuring/building.

The warmer independently builds the merged tree. It does not download a PR's
cache archive into shared scope. The first successful main-scoped warm may be
cold; an existing PR-scoped archive cannot seed it across that boundary. Merges
are not canceled or replaced by a shared concurrency group, so overlapping merge
events keep their own builds. Identical immutable cache keys may already exist
by post-save time; correctness never depends on a particular writer winning.

A successful earlier [PR-only run, 37788654266](https://github.com/aron98/openttd-rust/actions/runs/37788654266),
recorded 1,050 cacheable compiler calls: 1,046 misses and 4 hits, then uploaded
48,900,522 bytes (about 49 MB). This establishes a cold build and PR-scoped cache
upload, not a shared cache hit. The new warmer cannot be exercised by an open PR;
its activation, shared upload, and a future PR's cross-PR restoration remain
unverified until the workflow is merged and those runs are observed. No remote
warm-cache speedup is claimed.

## Precompiled headers and local use

OpenTTD enables precompiled headers. Cached CI builds pass
`CMAKE_DISABLE_PRECOMPILE_HEADERS=ON`, allowing ordinary translation units to be
cached without the relaxed checks documented for [ccache's PCH support](https://ccache.dev/manual/latest.html#_precompiled_headers).
Cold builds may therefore cost more than an uncached PCH build. Warm cache hits
can avoid compilation work, but there is no measured remote CI speedup yet.

Both setup scripts retain uncached compilation and PCH by default. Local opt-in:

```sh
export REFERENCE_COMPILER_LAUNCHER=ccache REFERENCE_DISABLE_PCH=ON
export CCACHE_DIR="$PWD/.reference/ccache" CCACHE_BASEDIR="$PWD"
export CCACHE_COMPILERCHECK=content CCACHE_MAXSIZE=2G CCACHE_SLOPPINESS=''
bash scripts/setup-reference.sh
REFERENCE_SOURCE="$PWD/.reference/OpenTTD" bash scripts/setup-snapshot-reference.sh
ccache --show-stats --verbose
```

Unset the two `REFERENCE_*` variables to restore the default launcher/PCH behavior
on the next setup invocation. Existing up-to-date object files may mean CMake does
not invoke the compiler at all; that is distinct from a ccache hit.

The launcher and PCH switches use the standard [CMake compiler-launcher](https://cmake.org/cmake/help/latest/variable/CMAKE_LANG_COMPILER_LAUNCHER.html)
and [PCH-disable](https://cmake.org/cmake/help/latest/variable/CMAKE_DISABLE_PRECOMPILE_HEADERS.html)
settings. No upstream build files are patched for caching.

## Local validation

On macOS ARM64 with Apple Clang and ccache 4.14.1, a real native CMake compilation
of `src/core/random_func.cpp` produced one cold miss and one direct hit after its
object was removed; the rebuilt object bytes were identical. Isolated copies of
that native source and header were then compiled with its generated native flags:
a semantic source edit and a header-layout edit each caused a miss and changed
the object hash; unchanged repeats hit and reproduced the exact object bytes.
The isolated sequence recorded three misses and two direct hits, with no lax
settings. The pinned source checkout remained clean.

The new orchestration is checked against the current workflow and composite-action
schemas, including rejection of invalid `cache-mode` values. Installed upstream
`actionlint` 1.7.12 does not recognize this documented GitHub key; that limitation
is recorded rather than suppressed. Suppression-free lint uses the `jactionlint`
fork v1.8.0, pinned at `d62e0904ff5a87765dc3e07eef4ebed3c02f05a9`, alongside
full SchemaStore validation. Bash syntax and ShellCheck cover extracted run steps.

GitHub's expression evaluator executes the condition extracted from the actual
warmer workflow against merged-main, unmerged, other-base, foreign-repository,
and wrong-event fixtures. The actual SHA guard rejects branch/PR refs and missing
or malformed SHAs. Shared compiler environment and rendered primary/fallback keys
are checked against both consumers and the previous cache configuration.

Raw native cache evidence remains under `.omo/evidence/ci-cpp-cache/`; orchestration
commands, schema snapshots, event fixtures, and validator results are under
`.omo/evidence/ci-cache-warmer/`. These local checks do not establish a completed
remote warmer or cross-PR cache reuse.
