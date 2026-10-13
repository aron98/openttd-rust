from typing import Final

COMPILER_INPUTS: Final = (
    "fixtures/content/contract-speed.grf",
    "fixtures/world/populated-v362.sav",
)

API: Final = (
    "all-owner-indices",
    "all-string-mappings",
    "ordered-writes",
    "reserve-bounds",
    "custom-reset",
)
LOAD: Final = (
    "deferred-and-overwrite",
    "reload-empty-custom-retained",
    "reload-reordered",
    "disabled-retained",
    "wrapped-and-invalid",
    "overwrite-reverse",
    "missing-custom",
    "builtin-unknown-empty",
    "custom-callback-aliases",
    "init-only-no-owner-write",
    "empty-configured-files",
    "zero-item-property",
    "maximum-count-wrap",
    "malformed-invalid-owner",
    "malformed-after-valid-record",
    "repeated-custom-owner",
    "same-local-distinct-grfid",
    "reload-same-config",
    "reload-remove-first",
    "reload-remove-second",
)
GUARDS: Final = (
    "menu",
    "replay",
    "subset",
    "reused",
    "non-save",
    "stale",
    "duplicate",
    "unarmed",
    "pending",
    "stale-read",
    "mixed",
    "reload-index",
    "reload-duplicate",
)
SELECTORS: Final = {
    "api": "content::grf::load_currency_api_native::original_currency_api_matrix",
    "load": "content::grf::load_currency_load_native::original_currency_load_matrix",
    "guards": "content::grf::load_currency_guards::original_currency_guards",
}
