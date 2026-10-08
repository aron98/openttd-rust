# CI reference-build cache

The interoperability job uses ccache for the C/C++ compilations in both original
OpenTTD reference builds. It always configures, builds, and runs the compatibility,
snapshot, landscape, and callback comparison matrices. A cache hit never skips a
build step or reuses a test result.

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
  workflow itself.

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

`actionlint`, `bash -n`, and ShellCheck validate the workflow/setup syntax. Raw
local commands, compiler output, hashes, and cache statistics are retained under
`.omo/evidence/ci-cpp-cache/`. These checks establish local cache behavior; they do
not claim Linux runner performance or a completed remote warm-cache CI run.
