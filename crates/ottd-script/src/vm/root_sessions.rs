//! Replay the captured passive host protocol through the single production VM loop.
use super::{Execution, Slot, Storage, Temporary, Vm};
use crate::{Program, Realm, RootEnvironment, Runner, Value, VmError};
use std::{error::Error, fmt::Write, fs, path::Path};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
struct Session {
    runners: Vec<Option<Runner>>,
    frames: Vec<Option<Vm<'static>>>,
    programs: Vec<Option<(usize, Program)>>,
    results: Vec<Option<Value>>,
    ids: Vec<usize>,
    next_id: usize,
}
fn render(value: &Value) -> Result<String> {
    Ok(match value {
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
fn bytes(hex: &str) -> Result<Vec<u8>> {
    if hex == "-" {
        return Ok(Vec::new());
    }
    hex.as_bytes()
        .chunks(2)
        .map(|pair| Ok(u8::from_str_radix(std::str::from_utf8(pair)?, 16)?))
        .collect()
}
fn number(fields: &[&str], index: usize) -> Result<usize> {
    Ok(fields.get(index).ok_or("missing index")?.parse()?)
}
fn dump(program: &Program, out: &mut String) -> Result<()> {
    writeln!(out, "stack {}", program.stack_size())?;
    for value in program.literals() {
        writeln!(out, "literal {}", render(value)?)?;
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
impl Session {
    fn new() -> Self {
        let root = Realm::new().empty_root();
        Self {
            runners: vec![
                Some(Runner::new(root.clone())),
                Some(Runner::new(root)),
                Some(Runner::new(Realm::new().empty_root())),
            ],
            frames: vec![None, None, None],
            programs: vec![None; 32],
            results: vec![None; 3],
            ids: vec![0, 0, 1],
            next_id: 2,
        }
    }
    fn runner(&self, id: usize) -> Result<&Runner> {
        if let Some(frame) = self.frames.get(id).ok_or("bad frame")? {
            return Ok(frame.runner.get());
        }
        self.runners
            .get(id)
            .and_then(Option::as_ref)
            .ok_or_else(|| "missing runner".into())
    }
    fn idle(&mut self, id: usize) -> Result<&mut Runner> {
        self.runners
            .get_mut(id)
            .and_then(Option::as_mut)
            .ok_or_else(|| "runner is active or released".into())
    }
    fn root(&self, id: usize) -> Result<&RootEnvironment> {
        Ok(&self.runner(id)?.root)
    }
    fn temp(&self, id: usize, out: &mut String) -> Result<()> {
        match &self.runner(id)?.temporary {
            Temporary::Scalar(value) => writeln!(out, "temp {}", render(value)?)?,
            Temporary::MainProgram { .. } => writeln!(out, "temp unsupported 134217984")?,
        }
        Ok(())
    }
    fn execute(&mut self, id: usize, credit: u32, out: &mut String) -> Result<()> {
        let Some(frame) = self.frames.get_mut(id).ok_or("bad frame")?.as_mut() else {
            writeln!(out, "idle")?;
            return Ok(());
        };
        let result = frame.resume(credit);
        let terminal = match result {
            Ok(Execution::Suspended) => {
                writeln!(
                    out,
                    "suspend {} {}",
                    frame.remaining_ops(),
                    frame.instruction_pointer()
                )?;
                for (slot, value) in frame.registers.iter().enumerate() {
                    match value {
                        Slot::Scalar(value) => writeln!(out, "frame {slot} {}", render(value)?)?,
                        Slot::Root(root) => {
                            assert!(root.same(&frame.runner.get().root));
                            writeln!(
                                out,
                                "frame {slot} root {}",
                                self.ids.get(id).ok_or("missing root id")?
                            )?;
                        }
                    }
                }
                false
            }
            Ok(Execution::Returned(value)) => {
                writeln!(out, "return {} {}", frame.remaining_ops(), render(&value)?)?;
                *self.results.get_mut(id).ok_or("missing result")? = Some(value);
                true
            }
            Err(error) => {
                assert_eq!(error, VmError::MissingIndex);
                writeln!(out, "runtime_error {}", frame.remaining_ops())?;
                true
            }
        };
        self.temp(id, out)?;
        if terminal {
            let frame = self
                .frames
                .get_mut(id)
                .ok_or("bad frame")?
                .take()
                .ok_or("no frame")?;
            let Storage::Owned(runner) = frame.runner else {
                return Err("test requires owned runner storage".into());
            };
            *self.runners.get_mut(id).ok_or("bad runner")? = Some(runner);
        }
        Ok(())
    }
    fn compile_command(
        &mut self,
        fields: &[&str],
        id: usize,
        base: &Path,
        out: &mut String,
    ) -> Result<()> {
        let slot = number(fields, 2)?;
        let feed = *fields.get(3).ok_or("missing feed")?;
        let source = fs::read(base.join(fields.get(4).ok_or("missing source")?))?;
        let source = if feed == "unsigned" {
            source
                .into_iter()
                .map(char::from)
                .collect::<String>()
                .into_bytes()
        } else {
            source
        };
        match self.idle(id)?.compile_bytes(&source) {
            Ok(program) => {
                writeln!(out, "compiled")?;
                dump(&program, out)?;
                *self.programs.get_mut(slot).ok_or("bad program slot")? = Some((id, program));
            }
            Err(error) => {
                assert_eq!(error.kind, crate::CompileErrorKind::ExpectedToken);
                writeln!(out, "compile_error")?;
            }
        }
        Ok(())
    }
    fn command(&mut self, line: &str, base: &Path, out: &mut String) -> Result<()> {
        let fields: Vec<_> = line.split_whitespace().collect();
        let op = *fields.first().ok_or("missing op")?;
        let id = number(&fields, 1)?;
        match op {
            "compile" => self.compile_command(&fields, id, base, out)?,
            "seed" => {
                let key = bytes(fields.get(2).ok_or("missing key")?)?;
                let text = *fields.get(4).ok_or("missing value")?;
                let value = match *fields.get(3).ok_or("missing type")? {
                    "integer" => Value::Integer(text.parse()?),
                    "float" => Value::Float(text.parse()?),
                    "null" => Value::Null,
                    "bool" => Value::Bool(text == "1"),
                    "string" => Value::String(self.root(id)?.realm.string(&bytes(text)?)),
                    _ => return Err("invalid scalar type".into()),
                };
                self.root(id)?.new_slot(&key, value);
                writeln!(out, "seeded")?;
            }
            "raw" => {
                let key = fields.get(2).ok_or("missing key")?;
                let value = self
                    .root(id)?
                    .raw_get(&bytes(key)?)
                    .map_or_else(|| Ok("absent".to_owned()), |value| render(&value))?;
                writeln!(out, "raw {key} {value}")?;
            }
            "run" => {
                let slot = number(&fields, 2)?;
                let (owner, program) = self
                    .programs
                    .get(slot)
                    .and_then(Option::as_ref)
                    .ok_or("no program")?;
                assert_eq!(*owner, id);
                let runner = self
                    .runners
                    .get_mut(id)
                    .ok_or("bad runner")?
                    .take()
                    .ok_or("runner busy")?;
                let vm = Vm::with_runner(program, Storage::Owned(runner))?;
                *self.results.get_mut(id).ok_or("bad result")? = None;
                *self.frames.get_mut(id).ok_or("bad frame")? = Some(vm);
                self.execute(id, fields.get(3).ok_or("missing credit")?.parse()?, out)?;
            }
            "advance" => self.execute(id, fields.get(2).ok_or("missing credit")?.parse()?, out)?,
            "refs" => {
                let text = fields.get(2).ok_or("missing bytes")?;
                writeln!(
                    out,
                    "refs {text} {}",
                    self.root(id)?.realm.owners(&bytes(text)?)
                )?;
            }
            "temp" => self.temp(id, out)?,
            "drop" => {
                *self
                    .programs
                    .get_mut(number(&fields, 2)?)
                    .ok_or("bad slot")? = None;
                writeln!(out, "dropped")?;
            }
            "clear-result" => {
                *self.results.get_mut(id).ok_or("bad result")? = None;
                writeln!(out, "result_released")?;
            }
            "empty" => {
                let root = self.root(id)?.realm.empty_root();
                self.idle(id)?.replace_root(root)?;
                *self.ids.get_mut(id).ok_or("bad root id")? = self.next_id;
                self.next_id = self.next_id.checked_add(1).ok_or("root id overflow")?;
                writeln!(out, "root {}", self.ids.get(id).ok_or("bad root id")?)?;
            }
            "share" => {
                let source = number(&fields, 2)?;
                let root = self.root(source)?.clone();
                self.idle(id)?.replace_root(root)?;
                *self.ids.get_mut(id).ok_or("bad root id")? =
                    *self.ids.get(source).ok_or("bad source id")?;
                writeln!(out, "root {}", self.ids.get(id).ok_or("bad root id")?)?;
            }
            "root" => writeln!(out, "root {}", self.ids.get(id).ok_or("bad root id")?)?,
            "release-child" => {
                assert_eq!(id, 1);
                *self.runners.get_mut(id).ok_or("bad runner")? = None;
                writeln!(out, "child_released")?;
            }
            _ => return Err("invalid test command".into()),
        }
        Ok(())
    }
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
        for name in [
            "unsupported-root",
            "unsupported-field",
            "unsupported-update",
            "unsupported-array",
        ] {
            let native = fs::read_to_string(base.join(format!("native/{name}-{feed}.txt")))?;
            assert!(native.lines().any(|line| line == "compiled"));
            let source = fs::read(base.join(format!("fixtures/{name}.nut")))?;
            assert_eq!(
                Realm::new()
                    .compile_bytes(&source)
                    .expect_err("closed root syntax domain")
                    .kind,
                crate::CompileErrorKind::UnsupportedSyntax
            );
        }
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
        runner.root.new_slot(b"len", Value::Integer(3));
        assert_eq!(
            runner.start(&delegate)?.resume(1000)?,
            Execution::Returned(Value::Integer(3))
        );
    }
    Ok(())
}
