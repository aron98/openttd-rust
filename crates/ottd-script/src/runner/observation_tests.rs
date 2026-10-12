//! Read-only snapshots preserve persistent payloads and the public borrowed API.
use crate::{Execution, Realm, Runner, Value, heap::observation::ValueKind};

#[test]
fn snapshots_preserve_borrowed_error_reset_and_collection_results() {
    let realm = Realm::new();
    let mut runner = Runner::new(realm.default_root().expect("root"));
    let original = runner.snapshot();
    assert_eq!(original.root.kind, ValueKind::Table);
    assert_eq!(original.error.kind, ValueKind::Null);
    assert_eq!(original.temporary.kind, ValueKind::Null);
    assert!(runner.compile("local = ;").is_err());
    let before = realm.heap.snapshot();
    let message = runner.native_error().expect("message");
    let bytes = message.as_bytes();
    let error = runner.native_error_value();
    let observed = runner.snapshot();
    assert_eq!(observed.error.kind, ValueKind::String);
    assert!(std::ptr::eq(error, runner.native_error_value()));
    assert!(std::ptr::eq(
        message,
        runner.native_error().expect("same message")
    ));
    assert_eq!(bytes, b"expected 'IDENTIFIER'");
    assert_eq!(before, realm.heap.snapshot());
    let program = runner.compile("return 2;").expect("success");
    drop(program);
    assert_eq!(observed, runner.snapshot());
    runner.reset_native_error();
    assert_eq!(original, runner.snapshot());
    assert_eq!(realm.heap.collect(), 0);
}

#[test]
fn observing_caught_aggregate_error_does_not_delay_retirement() {
    let realm = Realm::new();
    let mut runner = Runner::new(realm.default_root().expect("root"));
    let program = runner
        .compile("try { throw [7]; } catch(e) {} return 1;")
        .expect("source");
    let mut vm = runner.start(&program).expect("frame");
    assert_eq!(vm.resume(10000), Ok(Execution::Returned(Value::Integer(1))));
    drop(vm);
    drop(program);
    let error = runner.snapshot().error;
    assert_eq!(error.kind, ValueKind::Array);
    let before = realm.heap.snapshot();
    assert_eq!(runner.snapshot().error, error);
    assert_eq!(realm.heap.snapshot(), before);
    runner.reset_native_error();
    let id = error.identity.expect("actual array generation");
    let after = realm.heap.snapshot();
    let entry = after
        .entries
        .iter()
        .find(|item| item.identity == id)
        .expect("retired generation");
    assert_eq!(entry.life, None);
    assert_eq!(runner.snapshot().error.kind, ValueKind::Null);
}
