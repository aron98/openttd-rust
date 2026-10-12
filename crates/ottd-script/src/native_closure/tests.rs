//! Source-ledger checks; the fixture is independently extracted from pinned C++.
use super::{NativeClosure, NativeFunction, NativeRegistry};
use crate::{DefaultFunction, Realm, Value, VmError};
use std::{collections::BTreeSet, fmt::Write};

#[test]
fn all_registration_rows_match_the_pinned_source_ledger() {
    // Given: immutable source literals, independent of the runtime descriptors.
    let expected = include_str!("../../tests/native-registration-ledger.tsv");
    let mut actual = String::new();
    // When: all descriptors are read in their source registration order.
    for &registry in NativeRegistry::ALL {
        for (ordinal, row) in registry.registrations().iter().enumerate() {
            write!(
                actual,
                "{}\t{ordinal}\t{}\t{}\t{}\t{}\t",
                registry.name(),
                std::str::from_utf8(row.name()).expect("ASCII name"),
                row.function().source_symbol(),
                row.parameter_check(),
                row.typemask().unwrap_or("<none>")
            )
            .expect("format");
            actual.push('[');
            for (index, mask) in row.type_checks().iter().enumerate() {
                if index != 0 {
                    actual.push(',');
                }
                write!(actual, "{mask}").expect("format");
            }
            actual.push_str("]\n");
        }
    }
    // Then: names, aliases, order, signed arity, literal and compiled masks match.
    assert_eq!(actual, expected);
    assert_eq!(actual.lines().count(), 70);
    let functions = NativeRegistry::ALL
        .iter()
        .flat_map(|r| r.registrations())
        .map(|row| row.function().source_symbol())
        .collect::<BTreeSet<_>>();
    assert_eq!(functions.len(), 44);
}

#[test]
fn registered_objects_own_the_source_metadata_without_installing_tables() {
    // Given: source rows and a realm with no configured root.
    let realm = Realm::new();
    // When: each row creates an actual independent native closure.
    for row in NativeRegistry::ALL.iter().flat_map(|r| r.registrations()) {
        let closure = NativeClosure::registered(&realm, row).expect("registration");
        // Then: every public value agrees with the row, including empty owners.
        assert_eq!(closure.name().as_bytes(), row.name());
        assert_eq!(closure.function(), row.function());
        assert_eq!(closure.parameter_check(), row.parameter_check());
        assert_eq!(closure.type_checks(), row.type_checks());
        assert_eq!(closure.type_check_count(), row.type_checks().len());
        assert_eq!(closure.outer_value_count(), 0);
        assert_eq!(closure.environment(), Value::Null);
        closure.read(|data| assert_eq!(data.type_checks, row.type_checks()));
    }
    assert!(realm.heap.snapshot().chain.is_empty());
}

#[test]
fn cloned_metadata_has_independent_owned_masks_and_an_immutable_bridge() {
    // Given: every nonempty source mask vector, owned by an actual closure.
    let realm = Realm::new();
    for row in NativeRegistry::ALL
        .iter()
        .flat_map(|r| r.registrations())
        .filter(|row| !row.type_checks().is_empty())
    {
        let closure = NativeClosure::registered(&realm, row).expect("registration");
        // When: metadata is duplicated as in SQNativeClosure::Clone, then modified privately.
        let mut duplicate = closure.read(Clone::clone);
        closure.read(|data| {
            assert!(!std::ptr::eq(
                data.type_checks.as_ptr(),
                duplicate.type_checks.as_ptr()
            ));
        });
        *duplicate.type_checks.first_mut().expect("nonempty") = 0;
        duplicate.type_checks.push(123);
        // Then: the original vector and static descriptor are unchanged.
        closure.read(|data| assert_eq!(data.type_checks, row.type_checks()));
        assert_eq!(closure.type_checks(), row.type_checks());
        assert_ne!(duplicate.type_checks, row.type_checks());
        assert_eq!(duplicate.name.as_bytes(), row.name());
        assert!(duplicate.outer_values.is_empty());
    }
}

#[test]
fn append_and_push_share_code_but_have_distinct_object_identities() {
    // Given: the two real array aliases in the pinned registration list.
    let realm = Realm::new();
    let rows = NativeRegistry::Array.registrations();
    let append = rows
        .iter()
        .find(|row| row.name() == b"append")
        .expect("append");
    let push = rows.iter().find(|row| row.name() == b"push").expect("push");
    // When: two registration objects and one public HostRoot clone are created.
    let append = NativeClosure::registered(&realm, append).expect("append");
    let push = NativeClosure::registered(&realm, push).expect("push");
    let alias = append.clone();
    // Then: native aliases share only callback identity; public Clone shares the object.
    assert_eq!(
        append.function(),
        NativeFunction::Default(DefaultFunction::ArrayAppend)
    );
    assert_eq!(append.function(), push.function());
    assert!(!append.same_identity(&push));
    assert!(append.same_identity(&alias));
    let watch = append.watch();
    drop(append);
    assert!(watch().is_some());
    drop(alias);
    assert!(watch().is_none());
}

#[test]
fn bare_constructor_refuses_default_identity_without_allocating_an_object() {
    // Given: every registered default callback identity.
    let realm = Realm::new();
    for row in NativeRegistry::ALL.iter().flat_map(|r| r.registrations()) {
        if let NativeFunction::Default(function) = row.function() {
            // When: the legacy bare constructor is asked for an unsupported default.
            let result = NativeClosure::new(&realm, row.function());
            // Then: no fabricated base metadata or graph object is returned.
            assert_eq!(result, Err(VmError::UnsupportedNativeFunction(function)));
            assert!(realm.heap.snapshot().chain.is_empty());
        }
    }
}
