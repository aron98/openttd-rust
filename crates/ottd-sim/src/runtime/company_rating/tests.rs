//! Synthetic cache lifetime and isolated candidate ownership controls.
use super::CompanyRatingCaches;

#[test]
fn fresh_process_has_every_owner_slot_zero() {
    let cache = CompanyRatingCaches::fresh_process();
    assert_eq!(cache.parts(), &[[0; 10]; 15]);
}

#[test]
fn candidate_update_does_not_alias_committed_or_other_owner() {
    let mut committed = CompanyRatingCaches::fresh_process();
    committed.update(14, [-7; 10]).unwrap();
    let mut candidate = committed.clone();
    candidate.update(0, [31; 10]).unwrap();
    assert_eq!(committed.parts()[0], [0; 10]);
    assert_eq!(candidate.parts()[0], [31; 10]);
    assert_eq!(candidate.parts()[14], [-7; 10]);
}

#[test]
fn invalid_owner_is_rejected_without_changing_any_slot() {
    let mut cache = CompanyRatingCaches::fresh_process();
    let before = cache.clone();
    assert!(cache.update(15, [1; 10]).is_err());
    assert!(cache.update(u32::MAX, [1; 10]).is_err());
    assert_eq!(cache, before);
}

#[test]
fn owner_update_replaces_all_parts_and_retains_inactive_slots() {
    let mut cache = CompanyRatingCaches::fresh_process();
    cache.update(14, [-9; 10]).unwrap();
    cache.update(0, [1; 10]).unwrap();
    cache.update(0, [0; 10]).unwrap();
    assert_eq!(cache.parts()[0], [0; 10]);
    assert_eq!(cache.parts()[14], [-9; 10]);
}
