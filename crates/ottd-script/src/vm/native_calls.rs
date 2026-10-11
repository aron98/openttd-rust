//! Stable native-call test selectors backed by cohesive consumer scenarios.
mod aliases;
mod owners;
mod safety;

#[test]
fn host_entries_preserve_original_call_owner_and_argument_lifetimes() {
    owners::host_entries_preserve_original_call_owner_and_argument_lifetimes();
}

#[test]
fn returned_native_arrays_retain_fill_aliases_until_the_last_temporary_releases() {
    aliases::returned_native_arrays_retain_fill_aliases_until_the_last_temporary_releases();
}

#[test]
fn invalid_call_destination_preserves_prior_native_print_side_effects() {
    safety::invalid_call_destination_preserves_prior_native_print_side_effects();
}

#[test]
fn foreign_host_entries_fail_before_a_main_frame_is_installed() {
    safety::foreign_host_entries_fail_before_a_main_frame_is_installed();
}

#[test]
fn malformed_native_argument_ranges_fail_without_dispatching_the_body() {
    safety::malformed_native_argument_ranges_fail_without_dispatching_the_body();
}

#[test]
fn native_failure_preserves_aliases_mutation_and_reusable_runner_state() {
    aliases::native_failure_preserves_aliases_mutation_and_reusable_runner_state();
}

#[test]
fn call_created_cycles_release_on_realm_teardown() {
    owners::call_created_cycles_release_on_realm_teardown();
}

#[test]
fn unsupported_native_conversions_remain_explicit_after_argument_evaluation() {
    safety::unsupported_native_conversions_remain_explicit_after_argument_evaluation();
}

#[test]
fn call_temporary_retains_the_exact_unbound_callee_until_return() {
    safety::call_temporary_retains_the_exact_unbound_callee_until_return();
}
