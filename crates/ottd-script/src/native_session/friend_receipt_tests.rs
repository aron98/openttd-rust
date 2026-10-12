//! Genuine original-backed retained-friend graph comparison against the
//! native friend-host receipt; this runs only with the capture directory
//! provided.
use super::observation;
use super::receipts::{self, ReceiptKind};
use crate::{NativeFriendField, NativeSession};

#[test]
#[ignore = "Required native retained-friend graph; actual original capture directory must be provided"]
#[expect(
    clippy::too_many_lines,
    reason = "The whole original recipe is one retention narrative; splitting would fragment it"
)]
fn native_retained_friend_matches_the_original_friend_host_receipt() {
    let raw = receipts::receipt("friend-host");
    let mut session = NativeSession::open(256).expect("materialized session");
    let realm = session.realm().clone();
    realm.heap.install_retired_trace(128);
    let token = session.retain_friend(256).expect("retained friend");
    let friend_object = token.object;
    // `roots begin friend-before`: the friend vm line and its five fields,
    // observed through the scoped view of the leased payload.
    let before = {
        let view = session.friend(&token).expect("friend view");
        view.scalars()
    };
    let vm = receipts::vm_line_in(&raw, "friend-before");
    assert_eq!(
        before.references,
        usize::try_from(vm.references).expect("refs")
    );
    assert_eq!(before.is_root, vm.is_root == 1);
    assert_eq!(
        before.stack_size,
        usize::try_from(vm.stack_size).expect("size")
    );
    assert_eq!(before.top, vm.top);
    assert_eq!(before.stackbase, vm.stackbase);
    assert_eq!(before.vargs_size, usize::try_from(vm.vargs).expect("vargs"));
    assert_eq!(before.callsstacksize, vm.callsstack);
    assert_eq!(before.etraps, vm.etraps);
    assert_eq!(before.ops, vm.ops);
    let root = receipts::object(&raw, "friend-before", "root");
    assert_eq!(root.kind, ReceiptKind::Table);
    assert_eq!(
        before.persistent,
        [
            NativeFriendField::Null,
            NativeFriendField::Null,
            NativeFriendField::Null,
            NativeFriendField::Table {
                references: usize::try_from(root.references).expect("root refs"),
                extent: usize::try_from(root.extent).expect("root extent"),
            },
            NativeFriendField::Null,
        ],
        "error, handler, debug and temp stay Null; the root field co-owns the shared root table"
    );
    // The friend's `Init` re-registered the base closures into the shared
    // root table, so the fresh closures precede the friend in the
    // allocation-order chain exactly like occurrence zero.
    let snapshot = session.shared.snapshot();
    assert_eq!(snapshot.heap.chain.len(), 86);
    let labels = vec![('F', friend_object)];
    let wanted = observation::receipt_labels(&raw, 0);
    assert_eq!(
        observation::labeled_positions(&snapshot, &labels, &wanted),
        receipts::chain_positions(&raw, 0),
        "the swapped base closures precede the friend context"
    );
    // `roots collected primary-collected 0`: the retained friend survives
    // the collection marked through the RefTable and stays unfinalized.
    assert_eq!(
        session.collector().collect(),
        Ok(receipts::collected(&raw, "primary-collected"))
    );
    let snapshot = session.shared.snapshot();
    let wanted = observation::receipt_labels(&raw, 1);
    assert_eq!(
        observation::labeled_positions(&snapshot, &labels, &wanted),
        receipts::chain_positions(&raw, 1),
        "the rebuilt chain keeps the retained friend right after the primary subtree"
    );
    // `roots begin friend-after`: the friend was never finalized, so its
    // scalars and its shared root field are unchanged.
    let after = {
        let view = session.friend(&token).expect("friend view");
        view.scalars()
    };
    assert_eq!(
        after, before,
        "a host-rooted friend survives collection unfinalized"
    );
    let vm = receipts::vm_line_in(&raw, "friend-after");
    assert_eq!(
        after.references,
        usize::try_from(vm.references).expect("refs")
    );
    assert_eq!(after.is_root, vm.is_root == 1);
    // Releasing the host retention frees the friend (`free F`), and the
    // final collection rebuilds the base chain without it.
    drop(token);
    assert!(realm.heap.payload_address(friend_object).is_none());
    assert_eq!(
        session.collector().collect(),
        Ok(receipts::collected(&raw, "friend-released"))
    );
    let snapshot = session.shared.snapshot();
    assert_eq!(snapshot.heap.chain.len(), 85);
    let wanted = observation::receipt_labels(&raw, 3);
    assert_eq!(
        observation::labeled_positions(&snapshot, &[], &wanted),
        receipts::chain_positions(&raw, 3),
        "the friend left the rebuilt chain after its release"
    );
    session.close();
    // The teardown order mirrors the receipt's frees: the friend at its
    // host release, then the base graph's O and V at close.
    let retired = realm.heap.retired_identities();
    let friend_position = retired.iter().position(|id| {
        id.index == friend_object.index && id.generation == friend_object.generation
    });
    let root_position = observation::label_identity(&snapshot, 'O').and_then(|root| {
        retired
            .iter()
            .position(|id| id.index == root.index && id.generation == root.generation)
    });
    let primary_position = observation::label_identity(&snapshot, 'V').and_then(|primary| {
        retired
            .iter()
            .position(|id| id.index == primary.index && id.generation == primary.generation)
    });
    let (friend_position, root_position, primary_position) = (
        friend_position.expect("freed friend"),
        root_position.expect("freed root"),
        primary_position.expect("freed primary"),
    );
    assert!(friend_position < root_position && root_position < primary_position);
}
