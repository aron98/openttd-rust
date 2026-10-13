//! Genuine original-backed comparisons of the constructed idle graph against
//! the native receipts; these run only with the capture directory provided.
use super::NativeSession;
use super::context::RootMembership;
use super::observation::{self, chain_positions, label_identity};
use super::receipts::{self, ReceiptKind};
use crate::heap::observation::ObjectKind;

fn root_object_id(snapshot: &super::observation::SessionSnapshot) -> crate::heap::ObjectId {
    let identity = label_identity(snapshot, 'O').expect("root identity");
    crate::heap::ObjectId {
        index: identity.index,
        generation: identity.generation,
    }
}
fn labeled_positions(session: &NativeSession, labels: &[char]) -> Vec<(usize, char)> {
    chain_positions(&session.shared.snapshot())
        .into_iter()
        .filter(|(_, label)| labels.contains(label))
        .collect()
}
#[test]
#[ignore = "Required native idle graph; actual original capture directory must be provided"]
fn native_idle_construction_matches_the_original_shared_receipt() {
    let shared = receipts::receipt("shared");
    let idle = receipts::receipt("idle");
    let session = NativeSession::open(256).expect("materialized session");
    // Source allocation accounting: thirteen shared tables + root + primary
    // VM + 70 closures = 85 collectables, one native chain node each.
    let snapshot = session.shared.snapshot();
    assert_eq!(snapshot.heap.chain.len(), 85);
    assert_eq!(
        observation::kind_count(&snapshot.heap, ObjectKind::Table),
        14
    );
    assert_eq!(
        observation::kind_count(&snapshot.heap, ObjectKind::NativeClosure),
        70
    );
    // The allocation-order chain places every labeled identity exactly where
    // the original pre-collection snapshot does.
    let expected = receipts::chain_positions(&shared, 0);
    let shared_labels = expected.iter().map(|(_, label)| *label).collect::<Vec<_>>();
    assert_eq!(
        labeled_positions(&session, &shared_labels),
        expected,
        "every shared receipt label sits at the original allocation position"
    );
    // The idle receipt additionally labels the root table and the primary.
    let idle_expected = receipts::chain_positions(&idle, 0);
    let idle_labels = idle_expected
        .iter()
        .map(|(_, label)| *label)
        .collect::<Vec<_>>();
    assert_eq!(labeled_positions(&session, &idle_labels), idle_expected);
    // Idle primary scalars match the receipt's `roots vm` line.
    let vm = receipts::vm_line(&idle);
    let scalars = snapshot.vm.expect("scalars");
    assert_eq!(
        scalars.references,
        usize::try_from(vm.references).expect("refs")
    );
    assert_eq!(usize::try_from(vm.is_root).expect("is root"), 1);
    assert_eq!(
        scalars.stack_size,
        usize::try_from(vm.stack_size).expect("size")
    );
    assert_eq!(scalars.top, vm.top);
    assert_eq!(scalars.stackbase, vm.stackbase);
    assert_eq!(
        scalars.vargs_size,
        usize::try_from(vm.vargs).expect("vargs")
    );
    assert_eq!(scalars.callsstacksize, vm.callsstack);
    assert_eq!(scalars.etraps, vm.etraps);
    assert_eq!(scalars.ops, vm.ops);
    // Persistent idle fields: only the root table, with one owner.
    for (name, observed_name, expected, membership) in [
        ("error", "error", ReceiptKind::Null, RootMembership::Null),
        (
            "handler",
            "handler",
            ReceiptKind::Null,
            RootMembership::Null,
        ),
        ("debug", "debug", ReceiptKind::Null, RootMembership::Null),
        (
            "root",
            "root",
            ReceiptKind::Table,
            RootMembership::Object(root_object_id(&snapshot)),
        ),
        ("temporary", "temp", ReceiptKind::Null, RootMembership::Null),
    ] {
        let field = receipts::object(&idle, "idle", name);
        assert_eq!(field.kind, expected, "field {name}");
        let observed = snapshot
            .fields
            .iter()
            .find(|(field, _)| *field == observed_name)
            .expect("field");
        assert_eq!(observed.1, membership, "field {name}");
    }
    let root_field = receipts::object(&idle, "idle", "root");
    assert_eq!(root_field.references, 1);
    assert_eq!(
        usize::try_from(root_field.extent).expect("extent"),
        session
            .realm()
            .heap
            .table_extent(label_identity(&snapshot, 'O').expect("root"))
            .expect("root table")
    );
    session.close();
}
#[test]
#[ignore = "Required native idle graph; actual original capture directory must be provided"]
fn native_idle_shared_fields_match_the_original_registration_receipt() {
    let shared = receipts::receipt("shared");
    let session = NativeSession::open(256).expect("materialized session");
    let snapshot = session.shared.snapshot();
    for (label, name) in [
        ('R', "R"),
        ('C', "C"),
        ('M', "M"),
        ('T', "T"),
        ('A', "A"),
        ('S', "S"),
        ('N', "N"),
        ('F', "F"),
        ('G', "G"),
        ('H', "H"),
        ('K', "K"),
        ('I', "I"),
        ('W', "W"),
    ] {
        let field = receipts::shared_object(&shared, name);
        assert_eq!(field.kind, ReceiptKind::Table, "field {name}");
        assert_eq!(field.references, 1, "field {name}");
        let identity = label_identity(&snapshot, label).expect("label");
        assert_eq!(
            session.realm().heap.table_extent(identity),
            Some(usize::try_from(field.extent).expect("extent")),
            "field {name}"
        );
        let counts = observation::counts(&snapshot.heap, identity).expect("counts");
        assert_eq!(counts.references, 1, "field {name}");
    }
    session.close();
}
#[test]
#[ignore = "Required native idle collection; actual original capture directory must be provided"]
fn native_idle_collection_collects_nothing_and_rebuilds_the_source_chain() {
    let shared = receipts::receipt("shared");
    let idle = receipts::receipt("idle");
    let session = NativeSession::open(256).expect("materialized session");
    assert_eq!(
        session.collector().collect().expect("idle collection"),
        receipts::collected(&shared, "shared-collected")
    );
    assert_eq!(
        receipts::collected(&idle, "idle-collected"),
        session.collector().collect().expect("second collection")
    );
    // The post-collection chain is rebuilt in the native root-enqueue order:
    // the primary subtree first, then registry, consts, meta and the ten
    // default tables in generator/thread-before-closure order.
    let snapshot = session.shared.snapshot();
    assert_eq!(snapshot.heap.chain.len(), 85);
    let expected = receipts::chain_positions(&shared, 1);
    let shared_labels = expected.iter().map(|(_, label)| *label).collect::<Vec<_>>();
    assert_eq!(
        labeled_positions(&session, &shared_labels),
        expected,
        "the rebuilt chain follows the native root-enqueue order"
    );
    let idle_expected = receipts::chain_positions(&idle, 1);
    let idle_labels = idle_expected
        .iter()
        .map(|(_, label)| *label)
        .collect::<Vec<_>>();
    assert_eq!(labeled_positions(&session, &idle_labels), idle_expected);
    session.close();
}
#[test]
#[ignore = "Required native idle close order; actual original capture directory must be provided"]
fn native_session_close_order_matches_the_original_free_receipts() {
    let shared = receipts::receipt("shared");
    let idle = receipts::receipt("idle");
    let session = NativeSession::open(256).expect("materialized session");
    let realm = session.realm().clone();
    let snapshot = session.shared.snapshot();
    realm.heap.install_retired_trace(96);
    session.close();
    let retired = realm.heap.retired_identities();
    let labels = retired
        .iter()
        .filter_map(|identity| {
            snapshot
                .labels
                .iter()
                .find(|(_, candidate)| candidate == identity)
                .map(|(label, _)| *label)
        })
        .collect::<Vec<_>>();
    let receipt_frees = |raw: &str| -> Vec<char> {
        receipts::free_order(raw)
            .into_iter()
            .filter(|label| labels.contains(label))
            .collect()
    };
    let shared_free = receipt_frees(&shared);
    let shared_expected: Vec<char> = labels
        .iter()
        .copied()
        .filter(|label| receipts::free_order(&shared).contains(label))
        .collect();
    assert_eq!(
        shared_free, shared_expected,
        "the shared receipt frees registry, consts, meta, the VM, then the defaults in order"
    );
    let idle_free = receipt_frees(&idle);
    let idle_expected: Vec<char> = labels
        .iter()
        .copied()
        .filter(|label| receipts::free_order(&idle).contains(label))
        .collect();
    assert_eq!(
        idle_free, idle_expected,
        "the idle receipt frees the root table before the primary VM"
    );
    assert_eq!(
        labels,
        [
            'O', 'R', 'C', 'M', 'V', 'T', 'A', 'S', 'N', 'F', 'G', 'H', 'K', 'I', 'W'
        ]
    );
    let after = realm.heap.snapshot();
    assert!(after.chain.is_empty());
}
