//! Table delegate identity is independent of own slots and shallow clones.
use ottd_script::{Realm, Value, VmError};

#[test]
fn attaching_replacing_detaching_and_cloning_preserve_aliases() {
    let realm = Realm::new();
    let owner = realm.table().expect("owner");
    let parent = realm.table().expect("parent");
    parent
        .new_slot(Value::Integer(0), Value::Integer(7))
        .expect("slot");
    owner.set_delegate(Some(&parent)).expect("attach");
    assert_eq!(owner.delegate(), Some(parent.clone()));
    assert_eq!(owner.raw_get(Value::Integer(0)), Ok(None));
    let copy = owner.shallow_clone().expect("clone");
    assert!(!copy.same_identity(&owner));
    assert_eq!(copy.delegate(), Some(parent.clone()));
    owner.set_delegate(None).expect("detach");
    assert_eq!(owner.delegate(), None);
    assert_eq!(copy.delegate(), Some(parent));
}

#[test]
fn rejected_cycles_and_foreign_delegates_preserve_the_old_edge() {
    let realm = Realm::new();
    let a = realm.table().expect("A");
    let b = realm.table().expect("B");
    let c = realm.table().expect("C");
    a.set_delegate(Some(&b)).expect("A to B");
    b.set_delegate(Some(&c)).expect("B to C");
    assert_eq!(a.set_delegate(Some(&a)), Err(VmError::DelegateCycle));
    assert_eq!(c.set_delegate(Some(&a)), Err(VmError::DelegateCycle));
    assert_eq!(a.delegate(), Some(b.clone()));
    assert_eq!(b.delegate(), Some(c.clone()));
    assert_eq!(c.delegate(), None);
    let foreign = Realm::new().table().expect("foreign");
    assert_eq!(a.set_delegate(Some(&foreign)), Err(VmError::RealmMismatch));
    assert_eq!(a.delegate(), Some(b));
}
