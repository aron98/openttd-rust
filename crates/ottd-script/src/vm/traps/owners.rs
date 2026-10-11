//! Full native lifetime receipts distinguish the error root from a self-cycle edge.
use crate::{Realm, Runner, Temporary, Value};
use std::fmt::Write;

#[test]
fn last_error_retains_caught_and_uncaught_graphs_until_reset() {
    for (source, expected, environment) in [
        (
            "try{throw [7];\n}catch(e){}return 1;\n",
            include_str!("caught_owner.txt"),
            "OTTD_SCRIPT_TRAP_CAUGHT_OWNER",
        ),
        (
            "local a=[null];\na[0]=a;\ntry{throw a;\n}catch(e){}a=null;\nreturn 1;\n",
            include_str!("caught_cycle_owner.txt"),
            "OTTD_SCRIPT_TRAP_CYCLE_OWNER",
        ),
        (
            "throw [7];\n",
            include_str!("uncaught_array.txt"),
            "OTTD_SCRIPT_TRAP_UNCAUGHT_OWNER",
        ),
    ] {
        let realm = Realm::new();
        let mut runner = Runner::new(realm.default_root().expect("root"));
        let program = runner
            .compile_bytes_named(source.as_bytes(), c"trap-owner")
            .expect("source");
        let mut vm = runner.start(&program).expect("frame");
        let result = vm.resume(10000);
        assert!(vm.finished);
        let frames = vm.frames.len();
        let traps = vm.traps.len();
        assert!(vm.frames.is_empty());
        assert!(vm.traps.is_empty());
        drop(vm);
        let mut output = format!(
            "completed {} {frames} {traps} unsupported 134217792\n",
            u8::from(result.is_ok())
        );
        let Value::Array(array) = runner.native_error_value() else {
            panic!("array error");
        };
        let references = array.0.reference_probe();
        drop(program);
        drop(result);
        runner.temporary = Temporary::Value(Value::Null);
        realm.collect_cycles();
        writeln!(
            output,
            "error retains {} {}",
            u8::from(references().is_some()),
            references().unwrap_or(0)
        )
        .expect("format");
        runner.reset_native_error();
        realm.collect_cycles();
        writeln!(output, "error reset {}", u8::from(references().is_some())).expect("format");
        let expected = std::env::var_os(environment).map_or_else(
            || expected.to_owned(),
            |path| std::fs::read_to_string(path).expect("current native lifetime capture"),
        );
        assert_eq!(output, expected);
    }
}
