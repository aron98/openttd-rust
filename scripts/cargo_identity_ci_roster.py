from typing import Final

COMPILER_INPUT: Final = "crates/ottd-sim/src/content/grf/load_cargo_native_cases.json"
GUARD_INPUT: Final = "scripts/cargo-identity-ci-guards.json"
LAYOUT: Final = "scripts/cargo-identity-ci-layout.json"
SELECTOR: Final = (
    "content::grf::load_currency_session_tests::cargo_native::"
    "cargo_native_complete_raw_states"
)
CASES: Final = (
    "loader-baseline",
    "api-v8-small-chain",
    "api-version-6",
    "api-version-7",
    "api-version-8",
    "api-partial-and-invalid-first",
    "api-override-copy",
    "api-duplicates-validity-labels",
    "api-road-invalid-and-stage",
)
EVENT_COUNTS: Final = (12, 28, 26, 26, 26, 29, 24, 45, 34)
INCLUDED_COUNTS: Final = (1, 17, 15, 15, 15, 18, 13, 34, 23)
API_PHASES: Final = frozenset(
    {
        "api-command",
        "property-enter",
        "property-return",
        "road-owner-resolved",
        "api-finish",
    }
)
FOCUSED: Final = tuple(
    "content::grf::load_currency_session_tests::" + module + "::" + name
    for module, names in (
        (
            "cargo_baseline",
            (
                "cargo_red_existing_road08_control",
                "cargo_red_reserve_bitnum_accepts_native_slot12",
                "cargo_red_reserve_label_accepts_native_qaaa",
                "cargo_red_global_translation_accepts_native_explicit_table",
                "cargo_red_road_default_accepts_native_invalid_cargo",
                "cargo_red_identity_translation_road_chain_retains_native_owner",
            ),
        ),
        (
            "cargo_units",
            (
                "cargo_constructor_checks_budget_before_owner_allocation",
                "cargo_reset_preserves_two_distinct_inherited_standard_masks",
                "cargo_unknown_stops_same_action_and_retains_prior_slot_write",
                "cargo_invalid_id_stops_same_action_without_consuming_later_table",
            ),
        ),
    )
    for name in names
)
