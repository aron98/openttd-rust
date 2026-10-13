from typing import Final

API: Final = (
    "indices-rate",
    "indices-options",
    "indices-prefix",
    "indices-suffix",
    "indices-euro",
    "extended-indices",
    "rate-boundaries",
    "euro-boundaries",
    "options-bytes",
    "options-ignored-bits",
    "symbols-single-byte",
    "symbols-utf8",
    "reader-bounds",
    "custom-reset-values",
)
LOAD: Final = (
    "single-v1",
    "single-v2",
    "packed-properties",
    "deferred-and-overwrite",
    "overwrite-reverse",
    "custom-bytes",
    "wrapped-and-invalid",
    "maximum-count-wrap",
    "zero-items-all-values",
    "init-only-no-owner-write",
    "malformed-reserve-valid-first",
    "malformed-reserve-invalid-owner",
    "disabled-retained",
    "empty-configured-files",
    "activation-truncated-prefix",
    "activation-truncated-suffix",
    "reload-same",
    "reload-reordered",
    "reload-remove-first",
    "reload-remove-second",
    "reload-empty",
)
SELECTORS: Final = {
    "api": (
        "content::grf::load_currency_properties_api_cases::"
        "original_currency_property_api_matrix"
    ),
    "load": (
        "content::grf::load_currency_properties_load_cases::"
        "original_currency_property_load_matrix"
    ),
}
COMPILER_INPUT: Final = (
    "crates/ottd-sim/src/content/grf/load_currency_properties_pilot.json"
)
GUARD_INPUT: Final = "scripts/currency-properties-guards.json"
GUARDS: Final = (
    "encoding-false",
    "encoding-unknown",
    "encoding-missing",
    "encoding-without-fixture",
    "mixed-load-api",
    "mixed-api-reload",
    "wrong-property",
    "wide-property",
    "negative-first",
    "wide-first",
    "wide-count",
    "wrong-stage",
    "negative-byte",
    "wide-byte",
    "raw-too-long",
    "text-custom-in-byte-mode",
    "byte-custom-in-text-mode",
)
FOCUSED: Final = (
    "content::grf::load_currency_session_tests::properties::prefix_keeps_surrogate_bytes_when_native_decoder_accepts_them",
    "content::grf::load_currency_session_tests::property_units::legacy_projection_refuses_surrogate_bytes_without_replacement",
    "content::grf::load_currency_session_tests::property_units::repeated_value_writes_obey_cumulative_trace_budget",
    "content::grf::load_currency_session_tests::currency_adjacent_properties_remain_unsupported",
)
PROBES: Final = (
    "native-symbol",
    "rust-symbol",
    "false-control",
    "wrong-encoding",
    "one-sided-rng",
    "paired-lossy-symbol",
    "missing-api-case",
    "reordered-loader",
    "source",
    "compiled-input",
    "retained-executable",
    "wrong-cargo-selection",
)
