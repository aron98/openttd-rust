//! Always-on witnesses of the Runner-level native-session path: the
//! ordinary default-root construction, its session lifetime and the
//! bare-local friend contexts of ordinary runners.
use crate::heap::observation::{Identity, ObjectKind};
use crate::{Realm, Runner, Value};

fn live_tables(realm: &Realm) -> usize {
    realm
        .heap
        .snapshot()
        .entries
        .iter()
        .filter(|entry| entry.kind == Some(ObjectKind::Table) && entry.life.is_some())
        .count()
}
#[test]
fn ordinary_default_roots_share_one_materialized_session_root() {
    let realm = Realm::new();
    let first = realm.default_root().expect("first root");
    let second = realm.default_root().expect("second root");
    assert!(first.table.same_identity(&second.table));
    assert_eq!(first.table.len(), 8);
    assert_eq!(live_tables(&realm), 14);
    assert_eq!(realm.heap.collect(), 0);
    assert_eq!(live_tables(&realm), 14);
}
#[test]
fn the_last_configured_root_closes_the_session_and_a_later_root_rebuilds() {
    let realm = Realm::new();
    let root = realm.default_root().expect("root");
    assert_eq!(live_tables(&realm), 14);
    drop(root);
    let after = realm.heap.snapshot();
    assert!(
        after.chain.is_empty(),
        "the last configured root runs the source close sequence"
    );
    assert_eq!(live_tables(&realm), 0);
    let rebuilt = realm.default_root().expect("rebuilt root");
    assert_eq!(live_tables(&realm), 14);
    assert_eq!(realm.heap.collect(), 0);
    drop(rebuilt);
}
#[test]
fn replaced_runner_roots_publish_the_primary_membership_of_the_session() {
    let realm = Realm::new();
    let mut runner = Runner::new(realm.default_root().expect("root"));
    let replacement = realm.empty_root();
    let identity = replacement.table.0.observation_identity();
    runner.replace_root(replacement).expect("replace");
    let current = realm.default_root().expect("current root");
    assert_eq!(current.table.0.observation_identity(), identity);
    drop(current);
}
#[test]
fn a_local_friend_runner_retires_its_context_when_it_drops() {
    let realm = Realm::new();
    let runner = Runner::new(realm.default_root().expect("root"));
    let friend = runner.friend().expect("friend");
    let identity = Identity::from(
        friend
            .root
            .context()
            .expect("bound friend context")
            .identity(),
    );
    let snapshot = realm.heap.snapshot();
    assert_eq!(
        snapshot
            .entries
            .iter()
            .find(|entry| entry.identity == identity)
            .expect("friend context entry")
            .kind,
        Some(ObjectKind::Context)
    );
    drop(friend);
    let after = realm.heap.snapshot();
    assert!(
        after
            .entries
            .iter()
            .find(|entry| entry.identity == identity)
            .expect("retired friend generation")
            .life
            .is_none(),
        "the friend local's sole reference retires the context like native"
    );
    let root = realm.default_root().expect("root");
    assert!(matches!(
        root.table
            .raw_get(Value::String(realm.string(b"print")))
            .expect("lookup"),
        Some(Value::NativeClosure(_))
    ));
}
