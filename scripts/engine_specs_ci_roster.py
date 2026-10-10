from typing import Final

CASES: Final = (
    "loader-baseline",
    "api-dynamic-on",
    "api-dynamic-off",
    "loader-scalars",
    "loader-truncated",
    "loader-unknown",
    "loader-unknown-followup",
    "loader-recognized-prefix",
)
EVENT_COUNTS: Final = (10, 40, 40, 30, 35, 35, 20, 20)
SELECTOR: Final = (
    "content::grf::load_currency_session_tests::engine_ci::original_engine_spec_corpus"
)
COMPILER_INPUT: Final = "crates/ottd-sim/src/content/grf/load_engine_ci_cases.json"
GUARD_INPUT: Final = "scripts/engine-specs-ci-guards.json"
LAYOUT: Final = "scripts/engine-specs-ci-layout.json"
FOCUSED: Final = (
    "content::grf::load_currency_session_tests::engine_units::engine_owners_reset_matches_all_original_native_fields",
    "content::grf::load_currency_session_tests::engine_units::engine_partial_multi_owner_read_retains_first_write_and_second_allocation",
    "content::grf::load_currency_session_tests::engine_units::engine_unknown_multi_owner_property_allocates_all_before_disabling",
    "content::grf::load_currency_session_tests::engine_units::engine_trace_budget_remains_host_error",
)
PROBES: Final = (
    "rust-owner",
    "native-reader",
    "paired-owner",
    "missing-command",
    "native-rng",
    "baseline-path",
    "native-invocation",
    "cargo-unlocked",
    "source",
    "selector",
    "missing-case",
)
