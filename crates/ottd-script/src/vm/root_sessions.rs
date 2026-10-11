//! Replay the captured passive host protocol through the single production VM loop.
mod session;
use super::Execution;
use crate::{Program, Realm, Runner, Value, VmError};
use session::Session;
use std::{error::Error, fmt::Write, fs, path::Path};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
pub(super) fn render(value: &Value) -> Result<String> {
    Ok(match value {
        Value::NativeClosure(_) => "unsupported 134218240".to_owned(),
        Value::ScriptClosure(_) => "unsupported 134217984".to_owned(),
        Value::Table(_) => "unsupported 167772192".to_owned(),
        Value::Array(_) => "unsupported 134217792".to_owned(),
        Value::Null => "null".to_owned(),
        Value::Bool(value) => format!("bool {}", u8::from(*value)),
        Value::Integer(value) => format!("integer {value}"),
        Value::Float(bits) => format!("float {bits}"),
        Value::String(value) => {
            let mut out = format!("string {} ", value.as_bytes().len());
            for byte in value.as_bytes() {
                write!(out, "{byte:02x}")?;
            }
            out
        }
    })
}
pub(super) fn bytes(hex: &str) -> Result<Vec<u8>> {
    if hex == "-" {
        return Ok(Vec::new());
    }
    hex.as_bytes()
        .chunks(2)
        .map(|pair| Ok(u8::from_str_radix(std::str::from_utf8(pair)?, 16)?))
        .collect()
}
pub(super) fn number(fields: &[&str], index: usize) -> Result<usize> {
    Ok(fields.get(index).ok_or("missing index")?.parse()?)
}
pub(super) fn dump(program: &Program, out: &mut String) -> Result<()> {
    writeln!(out, "stack {}", program.stack_size())?;
    for value in program.literals() {
        writeln!(out, "literal {}", render(&value)?)?;
    }
    for i in program.instructions() {
        writeln!(
            out,
            "op {} {} {} {} {}",
            i.opcode, i.arg0, i.arg1, i.arg2, i.arg3
        )?;
    }
    Ok(())
}
fn projected(native: &str) -> String {
    native
        .lines()
        .map(|line| {
            if line.starts_with("compile_error ") {
                "compile_error".to_owned()
            } else if line.starts_with("runtime_error ") {
                line.split_whitespace()
                    .take(2)
                    .collect::<Vec<_>>()
                    .join(" ")
            } else {
                line.to_owned()
            }
        })
        .fold(String::new(), |mut out, line| {
            out.push_str(&line);
            out.push('\n');
            out
        })
}
fn replay(base: &Path, session: &Path, native: &Path) -> Result<()> {
    let mut state = Session::new();
    let mut output = "configured_empty_roots 0 0 1\n".to_owned();
    for (index, line) in fs::read_to_string(session)?
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .enumerate()
    {
        writeln!(output, "step {index} {line}")?;
        state.command(line, base, &mut output)?;
    }
    assert_eq!(
        output,
        projected(&fs::read_to_string(native)?),
        "{}",
        session.display()
    );
    Ok(())
}
#[test]
fn supported_root_sessions_match_native_frames_and_owners() -> Result<()> {
    let base = std::env::var_os("OTTD_SCRIPT_ROOT_CORPUS").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/roots"),
        std::path::PathBuf::from,
    );
    let families = [
        "unsupported-array",
        "read",
        "create-replace",
        "set",
        "missing-set",
        "missing-read",
        "null-set",
        "explicit",
        "assignment-result",
        "assignment-alias",
        "failure-persistence",
        "update",
        "unicode-string",
        "surrogate-string",
        "compile-failure",
        "live-read",
        "late-scalar",
        "inline-explicit",
        "shared-independent",
        "lifetime",
        "byte-key",
        "budget-every-step",
        "suspended-host-writes",
    ];
    for name in families {
        for feed in ["buffer", "unsigned", "utf8"] {
            replay(
                &base,
                &base.join(format!("fixtures/{name}-{feed}.session")),
                &base.join(format!("native/{name}-{feed}.txt")),
            )?;
        }
    }
    Ok(())
}
#[test]
fn compile_call_owner_and_root_replacement_match_native() -> Result<()> {
    let base = std::env::var_os("OTTD_SCRIPT_ROOT_CORPUS").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/roots"),
        std::path::PathBuf::from,
    );
    for feed in ["buffer", "unsigned", "utf8"] {
        replay(
            &base.join("ownership"),
            &base.join(format!("ownership/fixtures/compile-temp-{feed}.session")),
            &base.join(format!("ownership/compile-temp-{feed}.txt")),
        )?;
        replay(
            &base.join("replacement"),
            &base.join(format!("replacement/fixtures/{feed}.session")),
            &base.join(format!("replacement/{feed}.txt")),
        )?;
    }
    Ok(())
}
#[test]
fn native_success_outside_scalar_root_domain_is_explicitly_rejected() -> Result<()> {
    let base = std::env::var_os("OTTD_SCRIPT_ROOT_CORPUS").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/roots"),
        std::path::PathBuf::from,
    );
    for feed in ["buffer", "unsigned", "utf8"] {
        replay(
            &base,
            &base.join(format!("fixtures/unsupported-update-{feed}.session")),
            &base.join(format!("native/unsupported-update-{feed}.txt")),
        )?;
        let realm = Realm::new();
        let root = realm.empty_root();
        let program =
            realm.compile_bytes(&fs::read(base.join("fixtures/unsupported-root.nut"))?)?;
        let mut root_runner = Runner::new(root.clone());
        assert_eq!(
            root_runner.start(&program)?.resume(100),
            Ok(Execution::Returned(Value::Table(root.table())))
        );
        root.new_slot(b"score", Value::Integer(7))?;
        let mut field_runner = Runner::new(root);
        let field_source = fs::read(base.join("fixtures/unsupported-field.nut"))?;
        let field = field_runner.compile_bytes(&field_source)?;
        let native_field =
            fs::read_to_string(base.join(format!("native/unsupported-field-{feed}.txt")))?;
        let native_ops = native_field
            .lines()
            .filter(|line| line.starts_with("op "))
            .collect::<Vec<_>>()
            .join("\n");
        let actual_ops = field
            .instructions()
            .iter()
            .map(|i| {
                format!(
                    "op {} {} {} {} {}",
                    i.opcode, i.arg0, i.arg1, i.arg2, i.arg3
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(actual_ops, native_ops);
        assert_eq!(
            field_runner.start(&field)?.resume(1000),
            Err(VmError::UnsupportedReceiver)
        );
        let mut runner = Runner::new(Realm::new().empty_root());
        let reader = runner.compile("return Later;")?;
        runner.compile("enum Later { X=12 }")?;
        assert_eq!(
            runner.start(&reader)?.resume(1000),
            Err(VmError::UnsupportedRuntimeValue)
        );
        assert!(
            fs::read_to_string(base.join(format!("native/late-enum-{feed}.txt")))?
                .contains("return 998 unsupported 167772192")
        );
        let delegate = runner.compile("return len;")?;
        runner.compile("const len=4;")?;
        assert_eq!(
            runner.start(&delegate)?.resume(1000),
            Err(VmError::UnsupportedRuntimeValue)
        );
        assert!(
            fs::read_to_string(base.join(format!("native/delegate-precedence-{feed}.txt")))?
                .contains("return 998 unsupported 134218240")
        );
        runner.root.new_slot(b"len", Value::Integer(3))?;
        assert_eq!(
            runner.start(&delegate)?.resume(1000)?,
            Execution::Returned(Value::Integer(3))
        );
    }
    Ok(())
}

#[test]
fn script_closure_observation_keeps_its_distinct_native_object_tag() -> Result<()> {
    let realm = Realm::new();
    let mut runner = Runner::new(realm.default_root()?);
    let main = runner.compile("return function(){return 7;};")?;
    let mut vm = runner.start(&main)?;
    let Execution::Returned(result) = vm.resume(10000)? else {
        return Err("closure return required".into());
    };
    let actual = format!("return {} {}", vm.remaining_ops(), render(&result)?);
    let expected = include_str!("script-closure-type.txt")
        .lines()
        .find(|line| line.starts_with("return "))
        .ok_or("native closure return record")?;
    assert_eq!(actual, expected);
    Ok(())
}
