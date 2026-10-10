//! Compare real compiler output and table state against unmodified native captures.
use super::{
    Binding, Realm,
    corpus::{CASES, Domain},
};
use crate::{Program, Value};
use std::fmt::Write;
pub(super) fn value(value: &Value) -> Result<String, std::fmt::Error> {
    Ok(match value {
        Value::Null => "null".to_owned(),
        Value::Bool(value) => format!("bool {}", u8::from(*value)),
        Value::Integer(value) => format!("integer {value}"),
        Value::Float(bits) => format!("float {bits}"),
        Value::String(bytes) => {
            let mut output = format!("string {} ", bytes.as_bytes().len());
            for byte in bytes.as_bytes() {
                write!(output, "{byte:02x}")?;
            }
            output
        }
    })
}
pub(super) fn dump(program: &Program) -> Result<String, std::fmt::Error> {
    let mut output = format!("stack {}\n", program.stack_size());
    for literal in program.literals() {
        writeln!(output, "literal {}", value(literal)?)?;
    }
    for instruction in program.instructions() {
        writeln!(
            output,
            "op {} {} {} {} {}",
            instruction.opcode,
            instruction.arg0,
            instruction.arg1,
            instruction.arg2,
            instruction.arg3
        )?;
    }
    Ok(output)
}
pub(super) fn lookup(realm: &Realm, name: &str, member: &str) -> Result<String, std::fmt::Error> {
    let answer = match realm.constant(name) {
        None => "absent".to_owned(),
        Some(Binding::Scalar(scalar)) => {
            if member == "-" {
                value(&scalar.value())?
            } else {
                "not_enum".to_owned()
            }
        }
        Some(Binding::Enum(members)) => {
            if member == "-" {
                "enum".to_owned()
            } else {
                match members.get(member.as_bytes()) {
                    Some((_, scalar)) => value(&scalar.value())?,
                    None => "missing_member".to_owned(),
                }
            }
        }
    };
    Ok(format!("lookup {name} {member} {answer}\n"))
}
#[test]
fn native_buffer_corpus_matches_compilation_and_published_table()
-> Result<(), Box<dyn std::error::Error>> {
    for &(name, source, native, domain) in CASES {
        let realm = Realm::new();
        let compiled = realm.compile_bytes(source);
        if matches!(domain, Domain::RuntimeRootLookup) {
            let program = compiled.as_ref().map_err(|error| *error)?;
            assert_eq!(
                crate::Vm::new(program)?.resume(100),
                Err(crate::VmError::MissingIndex)
            );
        }
        assert_eq!(
            compiled.is_ok(),
            native.lines().any(|line| line == "compiled"),
            "{name}: {compiled:?}"
        );
        if let Ok(program) = compiled {
            let expected: String = native
                .lines()
                .filter(|line| {
                    line.starts_with("stack ")
                        || line.starts_with("literal ")
                        || line.starts_with("op ")
                })
                .fold(String::new(), |mut out, line| {
                    out.push_str(line);
                    out.push(char::from(10));
                    out
                });
            assert_eq!(dump(&program)?, expected, "{name}");
        }
        let mut observed = String::new();
        for (key, member) in [("K", "-"), ("E", "-"), ("E", "A"), ("E", "B"), ("E", "D")] {
            observed.push_str(&lookup(&realm, key, member)?);
        }
        let expected: String = native
            .lines()
            .filter(|line| line.starts_with("lookup "))
            .fold(String::new(), |mut out, line| {
                out.push_str(line);
                out.push(char::from(10));
                out
            });
        assert_eq!(observed, expected, "{name}");
    }
    Ok(())
}

#[test]
fn temporary_updates_match_native_suspension_results() -> Result<(), Box<dyn std::error::Error>> {
    let realm = Realm::new();
    let mut output = String::new();
    for source in [
        include_bytes!("../../../tests/constants/fixtures/compound.nut").as_slice(),
        include_bytes!("../../../tests/constants/fixtures/prefix.nut").as_slice(),
    ] {
        let program = realm.compile_bytes(source)?;
        let mut vm = crate::Vm::new(&program)?;
        for credit in [0, 2, 2, 2, 2, 2, 100] {
            match vm.resume(credit)? {
                crate::Execution::Suspended => writeln!(
                    output,
                    "suspend {} {}",
                    vm.remaining_ops(),
                    vm.instruction_pointer()
                )?,
                crate::Execution::Returned(result) => {
                    writeln!(output, "return {} {}", vm.remaining_ops(), value(&result)?)?;
                    break;
                }
            }
        }
        assert_eq!(lookup(&realm, "K", "-")?, "lookup K - integer 7\n");
    }
    let native = include_str!("../../../tests/constants/native/updates-session.txt");
    let expected: String = native
        .lines()
        .filter(|line| line.starts_with("suspend ") || line.starts_with("return "))
        .fold(String::new(), |mut out, line| {
            out.push_str(line);
            out.push(char::from(10));
            out
        });
    assert_eq!(output, expected);
    Ok(())
}

#[test]
fn declaration_timing_preserves_old_or_new_binding_at_native_byte_boundaries()
-> Result<(), Box<dyn std::error::Error>> {
    use crate::{CompileError, CompileErrorKind};
    let realm = Realm::new();
    realm.compile("const K=9;")?;
    for source in [b"const K=7;\xff".as_slice(), b"const K=7 \xff".as_slice()] {
        assert_eq!(
            realm.compile_bytes(source).map(|_| ()),
            Err(CompileError {
                offset: source.len().saturating_sub(1),
                kind: CompileErrorKind::InvalidCharacter
            })
        );
        assert_eq!(lookup(&realm, "K", "-")?, "lookup K - integer 9\n");
    }
    assert!(realm.compile("const K=7; local =;").is_err());
    assert_eq!(lookup(&realm, "K", "-")?, "lookup K - integer 7\n");
    realm.compile("enum E { A=9 }")?;
    for source in [b"enum E { A=".as_slice(), b"enum E { A }\xff".as_slice()] {
        assert!(realm.compile_bytes(source).is_err());
        assert_eq!(lookup(&realm, "E", "A")?, "lookup E A integer 9\n");
    }
    assert_eq!(
        realm.compile_bytes(b"enum E { A } \xff").map(|_| ()),
        Err(CompileError {
            offset: 13,
            kind: CompileErrorKind::InvalidCharacter
        })
    );
    assert_eq!(lookup(&realm, "E", "A")?, "lookup E A integer 0\n");
    Ok(())
}

fn owners(realm: &Realm, bytes: &[u8]) -> usize {
    realm
        .pool
        .borrow()
        .get(bytes)
        .map_or(0, std::rc::Weak::strong_count)
}
#[test]
fn names_values_and_partial_enum_scratch_have_distinct_owners()
-> Result<(), Box<dyn std::error::Error>> {
    let realm = Realm::new();
    realm.compile("const K=\"owned-value\";")?;
    assert_eq!(owners(&realm, b"K"), 1);
    assert_eq!(owners(&realm, b"owned-value"), 1);
    let program = realm.compile("return K;")?;
    realm.compile("const K=\"replacement-value\";")?;
    assert_eq!(owners(&realm, b"owned-value"), 1);
    let missing = Realm::new().compile("return K;")?;
    assert_eq!(
        crate::Vm::new(&missing)?.resume(100),
        Err(crate::VmError::MissingIndex)
    );
    {
        let mut vm = crate::Vm::new(&program)?;
        assert_eq!(vm.resume(0)?, crate::Execution::Suspended);
        let result = vm.resume(100)?;
        assert!(
            matches!(&result, crate::Execution::Returned(Value::String(bytes)) if bytes.as_bytes() == b"owned-value")
        );
        drop(result);
        assert_eq!(owners(&realm, b"owned-value"), 2);
    }
    assert_eq!(owners(&realm, b"owned-value"), 1);
    drop(program);
    assert_eq!(owners(&realm, b"owned-value"), 0);
    realm.compile("enum E { A=\"member-value\" }")?;
    assert_eq!(owners(&realm, b"E"), 1);
    assert_eq!(owners(&realm, b"A"), 1);
    assert!(realm.compile("enum E { B=\"scratch-value\", C=").is_err());
    assert_eq!(owners(&realm, b"scratch-value"), 0);
    assert_eq!(owners(&realm, b"B"), 0);
    assert_eq!(
        lookup(&realm, "E", "A")?,
        "lookup E A string 12 6d656d6265722d76616c7565\n"
    );
    realm.compile("const E=4;")?;
    assert_eq!(owners(&realm, b"member-value"), 0);
    assert_eq!(owners(&realm, b"A"), 0);
    Ok(())
}
