//! Public scalar-array behavior and explicit native-valid boundaries.
use ottd_script::{
    Array, Execution, Instruction, Program, Realm, Runner, Value, Vm, VmError, compile,
};
use std::error::Error;
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
fn returned(source: &str) -> Result<Value> {
    let program = compile(source)?;
    let Execution::Returned(value) = Vm::new(&program)?.resume(10_000)? else {
        return Err("unexpected suspension".into());
    };
    Ok(value)
}
fn array(value: Value) -> Result<Array> {
    match value {
        Value::Array(array) => Ok(array),
        _ => Err("expected array".into()),
    }
}
#[test]
fn scalar_storage_and_alias_identity() -> Result {
    let realm = Realm::new();
    let values = realm.array(vec![
        Value::Integer(1),
        Value::String(realm.string(b"x\0y")),
    ])?;
    let alias = values.clone();
    assert!(values.same_identity(&alias));
    assert_ne!(
        values,
        realm.array(vec![
            Value::Integer(1),
            Value::String(realm.string(b"x\0y"))
        ])?
    );
    alias.set(0, Value::Integer(7))?;
    assert_eq!(values.get(0), Some(Value::Integer(7)));
    assert_eq!(values.len(), 2);
    assert!(!values.is_empty());
    assert_eq!(values.get(-1), None);
    assert_eq!(values.get(2), None);
    Ok(())
}
#[test]
fn all_host_insertions_reject_container_edges_before_publication() -> Result {
    let realm = Realm::new();
    let values = realm.array(vec![Value::Integer(1)])?;
    assert_eq!(
        realm.array(vec![Value::Array(values.clone())]),
        Err(VmError::UnsupportedArrayElement)
    );
    assert_eq!(
        values.set(0, Value::Array(values.clone())),
        Err(VmError::UnsupportedArrayElement)
    );
    assert_eq!(values.get(0), Some(Value::Integer(1)));
    assert_eq!(
        values.set(8, Value::Array(values.clone())),
        Err(VmError::MissingIndex)
    );
    assert_eq!(values.len(), 1);
    Ok(())
}
#[test]
fn root_affinity_is_checked_without_cloning_or_replacing_foreign_arrays() -> Result {
    let first = Realm::new();
    let second = Realm::new();
    let root = first.empty_root();
    root.new_slot(b"data", Value::Integer(3))?;
    let foreign = second.array(vec![Value::Integer(9)])?;
    assert_eq!(
        root.new_slot(b"data", Value::Array(foreign.clone())),
        Err(VmError::RealmMismatch)
    );
    assert_eq!(
        root.set_existing(b"data", Value::Array(foreign)),
        Err(VmError::RealmMismatch)
    );
    assert_eq!(root.raw_get(b"data"), Some(Value::Integer(3)));
    let local = first.array(vec![Value::Integer(1)])?;
    root.new_slot(b"data", Value::Array(local.clone()))?;
    assert_eq!(array(root.raw_get(b"data").ok_or("missing data")?)?, local);
    Ok(())
}
#[test]
fn returned_array_and_string_outlive_all_context_wrappers() -> Result {
    let result = returned("return [\"owned\", 1];")?;
    let data = array(result)?;
    assert_eq!(
        data.get(0),
        Some(Value::String(Realm::new().string(b"owned")))
    );
    data.set(1, Value::Integer(7))?;
    assert_eq!(data.get(1), Some(Value::Integer(7)));
    Ok(())
}
#[test]
fn one_program_allocates_fresh_arrays_on_each_execution() -> Result {
    let program = compile("return [1];")?;
    let Execution::Returned(first) = Vm::new(&program)?.resume(100)? else {
        return Err("suspend".into());
    };
    let Execution::Returned(second) = Vm::new(&program)?.resume(100)? else {
        return Err("suspend".into());
    };
    assert_ne!(array(first)?, array(second)?);
    assert!(program.literals().is_empty());
    Ok(())
}
#[test]
fn arrays_are_not_mutable_compiler_literals() -> Result {
    let value = Value::Array(Realm::new().array(vec![])?);
    assert_eq!(
        Program::from_parts(2, vec![value], vec![]),
        Err(VmError::InvalidBytecode)
    );
    Ok(())
}
#[test]
fn actual_native_scalar_idioms_and_defined_float_indices() -> Result {
    for (source, expected) in [
        (
            "local score=[12,4,9]; local best=0; for(local i=1;i<3;i++){ if(score[i]<score[best]) best=i; } return best;",
            Value::Integer(1),
        ),
        (
            "local a=[1,2]; a[0]=(a[1]=9); return a[0]+a[1];",
            Value::Integer(18),
        ),
        (
            "local a=[7,9]; return a[-0.5]+a[0.9]+a[1.9];",
            Value::Integer(23),
        ),
        (
            "local a=[]; return a ? typeof a : \"false\";",
            Value::String(Realm::new().string(b"array")),
        ),
        ("if(false){local a=[[1]];}\nreturn 7;", Value::Integer(7)),
    ] {
        assert_eq!(returned(source)?, expected, "{source}");
    }
    Ok(())
}
#[test]
fn nested_arrays_are_reached_runtime_boundaries_not_lexer_policy() -> Result {
    for source in ["return [[1]];", "local a=[null]; a[0]=a; return a;"] {
        let program = compile(source)?;
        assert_eq!(
            Vm::new(&program)?.resume(100),
            Err(VmError::UnsupportedArrayElement)
        );
    }
    let program = compile("local a=[1]; a[2]=[3]; return a;")?;
    assert_eq!(Vm::new(&program)?.resume(100), Err(VmError::MissingIndex));
    Ok(())
}
#[test]
fn unsupported_runtime_domains_and_actual_missing_indices_stay_distinct() -> Result {
    for (source, expected) in [
        ("local a=[]; return a<a;", VmError::UnsupportedArrayOrdering),
        ("return \"\"+[];", VmError::UnsupportedRuntimeValue),
        ("return [1][\"len\"];", VmError::UnsupportedRuntimeValue),
        ("return [1][\"absent\"];", VmError::MissingIndex),
        ("return [1][true];", VmError::MissingIndex),
        ("local a=[1]; a[0] <- 9;", VmError::OperandType),
        ("local a=[1]; a[true]=2;", VmError::OperandType),
        ("return \"x\"[0];", VmError::UnsupportedReceiver),
        (
            "local a=[1]; return a[1.0/0.0];",
            VmError::UnsupportedIndexConversion,
        ),
    ] {
        let program = compile(source)?;
        assert_eq!(Vm::new(&program)?.resume(100), Err(expected), "{source}");
    }
    Ok(())
}
#[test]
fn float_index_conversion_covers_defined_i64_edge_and_nonfinite_bits() -> Result {
    let realm = Realm::new();
    let root = realm.empty_root();
    root.new_slot(b"a", Value::Array(realm.array(vec![Value::Integer(7)])?))?;
    let mut runner = Runner::new(root.clone());
    let program = runner.compile("return a[index];")?;
    for (bits, expected) in [
        (0x0000_0000, Ok(Execution::Returned(Value::Integer(7)))),
        (0x8000_0000, Ok(Execution::Returned(Value::Integer(7)))),
        (0xbf00_0000, Ok(Execution::Returned(Value::Integer(7)))),
        (0xdf00_0000, Err(VmError::MissingIndex)),
        (0x5f00_0000, Err(VmError::UnsupportedIndexConversion)),
        (0xdf00_0001, Err(VmError::UnsupportedIndexConversion)),
        (0x7f80_0000, Err(VmError::UnsupportedIndexConversion)),
        (0xff80_0000, Err(VmError::UnsupportedIndexConversion)),
        (0x7fc0_0001, Err(VmError::UnsupportedIndexConversion)),
    ] {
        root.new_slot(b"index", Value::Float(bits))?;
        assert_eq!(runner.start(&program)?.resume(100), expected, "{bits:08x}");
    }
    Ok(())
}
#[test]
fn malformed_collection_bytecode_fails_without_unchecked_access() -> Result {
    for instruction in [
        Instruction {
            opcode: 0x1f,
            arg0: 0,
            arg1: 1,
            arg2: 0,
            arg3: 0,
        },
        Instruction {
            opcode: 0x1f,
            arg0: 9,
            arg1: 1,
            arg2: 0,
            arg3: 0,
        },
        Instruction {
            opcode: 0x1f,
            arg0: 1,
            arg1: -1,
            arg2: 0,
            arg3: 0,
        },
        Instruction {
            opcode: 0x1f,
            arg0: 1,
            arg1: 1,
            arg2: 1,
            arg3: 0,
        },
        Instruction {
            opcode: 0x20,
            arg0: 0,
            arg1: 1,
            arg2: 0,
            arg3: 0,
        },
        Instruction {
            opcode: 0x20,
            arg0: 1,
            arg1: 1,
            arg2: 0,
            arg3: 0,
        },
    ] {
        let program = Program::from_parts(2, vec![], vec![instruction])?;
        assert_eq!(Vm::new(&program)?.resume(10), Err(VmError::InvalidBytecode));
    }
    for (arg1, arg2, arg3) in [(1, 0, 255), (0, 255, 0), (9, 0, 0), (0, 255, 255)] {
        let program = Program::from_parts(
            2,
            vec![],
            vec![
                Instruction {
                    opcode: 0x1f,
                    arg0: 1,
                    arg1: 0,
                    arg2: 0,
                    arg3: 0,
                },
                Instruction {
                    opcode: 0x20,
                    arg0: 1,
                    arg1,
                    arg2,
                    arg3,
                },
            ],
        )?;
        assert_eq!(Vm::new(&program)?.resume(10), Err(VmError::InvalidBytecode));
    }
    Ok(())
}
