//! A closure survives its compiling main and runs through a later host entry.
use super::tests::expected;
use crate::{Execution, Realm, Runner, Temporary, Value};
use std::{fmt::Write, rc::Rc};

#[test]
fn persistent_closure_frames_and_late_errors_match_original_owners() {
    for failure in [false, true] {
        let realm = Realm::new();
        let root = realm.default_root().expect("root");
        let mut runner = Runner::new(root.clone());
        let main = runner
            .compile_bytes_named(
                b"held<-{x=1};\nsaved<-function():(held){return held;};\nreturn 0;",
                c"first",
            )
            .expect("compile");
        assert_eq!(
            runner.start(&main).expect("start").resume(10000),
            Ok(Execution::Returned(Value::Integer(0)))
        );
        let Some(Value::ScriptClosure(saved)) = root.raw_get(b"saved") else {
            panic!("saved closure");
        };
        let leaf = root.delete_slot(b"held").expect("held slot");
        let Value::Table(leaf) = leaf else {
            panic!("held table");
        };
        let leaf_watch = leaf.watch();
        let leaf_refs = leaf.0.reference_probe();
        let closure_watch = saved.0.watch();
        let proto_watch = saved.read(|data| Rc::downgrade(&data.prototype));
        drop(leaf);
        drop(main);
        let mut output = String::new();
        writeln!(
            output,
            "persisted {} {} {}",
            saved.0.references(),
            proto_watch.strong_count(),
            leaf_refs().expect("live table")
        )
        .expect("format");
        let source = if failure {
            "local f=::saved;\ndelete ::saved;\nlocal t=f();\nt.x=9;\nassert(false);\nreturn t;"
        } else {
            "local f=::saved;\ndelete ::saved;\nlocal t=f();\nt.x=9;\nreturn t;"
        };
        let main = runner
            .compile_bytes_named(source.as_bytes(), c"second")
            .expect("second compile");
        let mut vm = runner.start(&main).expect("second start");
        let result = resume(&mut vm, &saved, &leaf_refs, &mut output);
        drop(vm);
        let value = leaf_watch()
            .expect("live table")
            .raw_get(Value::String(realm.string(b"x")))
            .expect("lookup")
            .expect("slot");
        let Value::Integer(value) = value else {
            panic!("mutation integer");
        };
        writeln!(output, "mutation integer {value}").expect("format");
        drop(main);
        drop(saved);
        writeln!(
            output,
            "callee released {} {} {}",
            u8::from(closure_watch().is_some()),
            u8::from(proto_watch.upgrade().is_some()),
            leaf_refs().unwrap_or(0)
        )
        .expect("format");
        drop(result);
        runner.temporary.set(Temporary::Value(Value::Null));
        writeln!(output, "final {}", u8::from(leaf_watch().is_some())).expect("format");
        let (variable, fixture) = if failure {
            (
                "OTTD_SCRIPT_CLOSURE_LATE_ERROR",
                include_str!("late-error.txt"),
            )
        } else {
            (
                "OTTD_SCRIPT_CLOSURE_PERSISTENCE",
                include_str!("persistence.txt"),
            )
        };
        assert_eq!(output, expected(variable, fixture));
    }
}

fn resume(
    vm: &mut crate::Vm<'_>,
    saved: &crate::ScriptClosure,
    leaf_refs: &impl Fn() -> Option<usize>,
    output: &mut String,
) -> Option<Value> {
    for credit in [0, 1, 2, 4, 0, 2, 2, 2, 2, 2, 2, 2, 2, 16, 32, 10000] {
        match vm.resume(credit) {
            Ok(Execution::Suspended) => {
                let active = vm.active_prototype().is_some_and(|active| {
                    saved.read(|data| Rc::ptr_eq(&data.prototype, &active.0))
                });
                let (base, top, count) = vm.frame_position().expect("coordinates");
                writeln!(
                    output,
                    "suspended {} {} {} {} {} {} {} {} {}",
                    vm.remaining_ops(),
                    u8::from(active),
                    vm.instruction_pointer(),
                    count,
                    base,
                    top,
                    saved.0.references(),
                    saved.read(|data| Rc::strong_count(&data.prototype)),
                    leaf_refs().expect("live table")
                )
                .expect("format");
            }
            Ok(Execution::Returned(value)) => {
                writeln!(
                    output,
                    "returned {} {} {} {}",
                    vm.remaining_ops(),
                    saved.0.references(),
                    saved.read(|data| Rc::strong_count(&data.prototype)),
                    leaf_refs().expect("live table")
                )
                .expect("format");
                return Some(value);
            }
            Err(error) => {
                assert_eq!(error, crate::VmError::NativeCall);
                let message =
                    std::str::from_utf8(vm.native_error().expect("native message").as_bytes())
                        .expect("utf8");
                writeln!(
                    output,
                    "failed {} {} {} {} {}",
                    vm.remaining_ops(),
                    saved.0.references(),
                    saved.read(|data| Rc::strong_count(&data.prototype)),
                    leaf_refs().expect("live table"),
                    message
                )
                .expect("format");
                break;
            }
        }
    }
    None
}
