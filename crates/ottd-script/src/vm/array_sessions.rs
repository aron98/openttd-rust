//! Array-aware native host sessions replayed through the production VM.
use super::root_sessions::{bytes, dump, number, render};
use super::{Execution, Slot, Storage, Temporary, Vm};
use crate::{Array, CompileErrorKind, Program, Realm, RootEnvironment, Runner, Value, VmError};
use std::{error::Error, fmt::Write, fs, path::Path};
mod foreach;
type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
struct Session {
    runners: Vec<Option<Runner>>,
    frames: Vec<Option<Vm<'static>>>,
    programs: Vec<Option<(usize, Program)>>,
    results: Vec<Option<Value>>,
    handles: Vec<Option<Array>>,
    labels: Vec<Box<dyn Fn() -> Option<Array>>>,
    ids: Vec<usize>,
    next_id: usize,
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
            handles: vec![None; 32],
            labels: Vec::new(),
            ids: vec![0, 0, 1],
            next_id: 2,
        }
    }
    fn runner(&self, id: usize) -> Result<&Runner> {
        if let Some(frame) = self.frames.get(id).ok_or("frame index")? {
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
            .ok_or_else(|| "runner busy/released".into())
    }
    fn root(&self, id: usize) -> Result<&RootEnvironment> {
        Ok(&self.runner(id)?.root)
    }
    fn handle(&self, slot: usize) -> Result<&Array> {
        self.handles
            .get(slot)
            .and_then(Option::as_ref)
            .ok_or_else(|| "missing array handle".into())
    }
    fn label(&mut self, array: &Array) -> usize {
        if let Some(index) = self
            .labels
            .iter()
            .position(|probe| probe().is_some_and(|old| old.same_identity(array)))
        {
            return index.saturating_add(1);
        }
        self.labels.push(Box::new(array.watch()));
        self.labels.len()
    }
    fn value(&mut self, value: &Value) -> Result<String> {
        match value {
            Value::Array(array) => Ok(format!(
                "array {} length {}",
                self.label(array),
                array.len()
            )),
            Value::Null
            | Value::Bool(_)
            | Value::Integer(_)
            | Value::Float(_)
            | Value::String(_) => render(value),
        }
    }
    fn temp(&mut self, id: usize, out: &mut String) -> Result {
        let value = match &self.runner(id)?.temporary {
            Temporary::Value(value) => Some(value.clone()),
            Temporary::MainProgram { .. } => None,
        };
        if let Some(value) = value {
            writeln!(out, "temp {}", self.value(&value)?)?;
        } else {
            writeln!(out, "temp unsupported 134217984")?;
        }
        Ok(())
    }
    fn snapshot(&mut self, out: &mut String) -> Result {
        for index in 0..self.labels.len() {
            let Some(array) = self.labels.get(index).ok_or("label")?() else {
                writeln!(out, "array_dead {}", index.saturating_add(1))?;
                continue;
            };
            let label = index.saturating_add(1);
            writeln!(out, "array_state {label} length {}", array.len())?;
            for element in 0..array.len() {
                let value = array.get(i64::try_from(element)?).ok_or("element")?;
                writeln!(out, "element {label} {element} {}", self.value(&value)?)?;
            }
        }
        Ok(())
    }
    fn scalar(&self, id: usize, fields: &[&str], kind: usize) -> Result<Value> {
        let text = *fields.get(kind.saturating_add(1)).ok_or("payload")?;
        Ok(match *fields.get(kind).ok_or("kind")? {
            "null" => Value::Null,
            "bool" => Value::Bool(text == "1"),
            "integer" => Value::Integer(text.parse()?),
            "float" => Value::Float(text.parse()?),
            "string" => Value::String(self.root(id)?.realm.string(&bytes(text)?)),
            _ => return Err("unsupported host scalar".into()),
        })
    }
    fn execute(&mut self, id: usize, credit: u32, out: &mut String) -> Result {
        let frame = self
            .frames
            .get_mut(id)
            .and_then(Option::as_mut)
            .ok_or("inactive frame")?;
        let result = frame.resume(credit);
        let remaining = frame.remaining_ops();
        let ip = frame.instruction_pointer();
        let registers = frame.registers.clone();
        let terminal = match result {
            Ok(Execution::Suspended) => {
                writeln!(out, "suspend {remaining} {ip}")?;
                for (index, slot) in registers.iter().enumerate() {
                    match slot {
                        Slot::Value(value) => {
                            writeln!(out, "frame {index} {}", self.value(value)?)?;
                        }
                        Slot::Root(root) => {
                            assert!(root.same(self.root(id)?));
                            writeln!(
                                out,
                                "frame {index} root {}",
                                self.ids.get(id).ok_or("root id")?
                            )?;
                        }
                    }
                }
                false
            }
            Ok(Execution::Returned(value)) => {
                writeln!(out, "return {remaining} {}", self.value(&value)?)?;
                *self.results.get_mut(id).ok_or("result")? = Some(value);
                true
            }
            Err(error) => {
                assert!(
                    matches!(
                        error,
                        VmError::MissingIndex | VmError::OperandType | VmError::DivisionByZero
                    ),
                    "unexpected boundary: {error:?}"
                );
                writeln!(out, "runtime_error {remaining} {error:?}")?;
                true
            }
        };
        self.temp(id, out)?;
        if terminal {
            let frame = self
                .frames
                .get_mut(id)
                .ok_or("frame")?
                .take()
                .ok_or("no frame")?;
            let Storage::Owned(runner) = frame.runner else {
                return Err("borrowed test runner".into());
            };
            *self.runners.get_mut(id).ok_or("runner")? = Some(runner);
        }
        Ok(())
    }
    fn program_command(
        &mut self,
        op: &str,
        id: usize,
        f: &[&str],
        base: &Path,
        out: &mut String,
    ) -> Result {
        match op {
            "compile" => {
                let slot = number(f, 2)?;
                let source = fs::read(base.join(f.get(4).ok_or("path")?))?;
                let source = if f.get(3) == Some(&"unsigned") {
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
                        *self.programs.get_mut(slot).ok_or("program")? = Some((id, program));
                    }
                    Err(error) => {
                        assert_eq!(error.kind, CompileErrorKind::ExpectedToken);
                        writeln!(out, "compile_error")?;
                    }
                }
            }
            "run" => {
                let slot = number(f, 2)?;
                let (owner, program) = self
                    .programs
                    .get(slot)
                    .and_then(Option::as_ref)
                    .ok_or("program")?;
                assert_eq!(*owner, id);
                let runner = self
                    .runners
                    .get_mut(id)
                    .ok_or("runner")?
                    .take()
                    .ok_or("busy runner")?;
                *self.results.get_mut(id).ok_or("result")? = None;
                *self.frames.get_mut(id).ok_or("frame")? =
                    Some(Vm::with_runner(program, Storage::Owned(runner))?);
                self.execute(id, f.get(3).ok_or("credit")?.parse()?, out)?;
            }
            "advance" => self.execute(id, f.get(2).ok_or("credit")?.parse()?, out)?,
            "drop" => {
                *self.programs.get_mut(number(f, 2)?).ok_or("program")? = None;
                writeln!(out, "program_released")?;
            }
            "clear-result" => {
                *self.results.get_mut(id).ok_or("result")? = None;
                writeln!(out, "result_released")?;
            }
            _ => return Err("program command".into()),
        }
        Ok(())
    }
    fn value_command(&mut self, op: &str, id: usize, f: &[&str], out: &mut String) -> Result {
        match op {
            "make-array" => {
                let slot = number(f, 2)?;
                let count = number(f, 3)?;
                let values = (0..count)
                    .map(|n| self.scalar(id, f, 4_usize.saturating_add(n.saturating_mul(2))))
                    .collect::<Result<Vec<_>>>()?;
                let array = self.root(id)?.realm.array(values)?;
                writeln!(
                    out,
                    "handle {slot} {}",
                    self.value(&Value::Array(array.clone()))?
                )?;
                *self.handles.get_mut(slot).ok_or("handle")? = Some(array);
            }
            "hold-result" | "hold-root" => {
                let slot = number(f, 2)?;
                let value = if op == "hold-result" {
                    self.results
                        .get(id)
                        .and_then(Option::as_ref)
                        .ok_or("result")?
                        .clone()
                } else {
                    self.root(id)?
                        .raw_get(&bytes(f.get(3).ok_or("key")?)?)
                        .ok_or("root value")?
                };
                let Value::Array(array) = value else {
                    return Err("nonarray handle".into());
                };
                writeln!(
                    out,
                    "handle {slot} {}",
                    self.value(&Value::Array(array.clone()))?
                )?;
                *self.handles.get_mut(slot).ok_or("handle")? = Some(array);
            }
            "drop-handle" => {
                let slot = number(f, 2)?;
                *self.handles.get_mut(slot).ok_or("handle")? = None;
                writeln!(out, "handle_released {slot}")?;
            }
            "seed" | "root-store-array" => {
                let key = bytes(f.get(2).ok_or("key")?)?;
                let value = if op == "seed" {
                    self.scalar(id, f, 3)?
                } else {
                    Value::Array(self.handle(number(f, 3)?)?.clone())
                };
                self.root(id)?.new_slot(&key, value)?;
                writeln!(out, "seeded")?;
            }
            "raw" => {
                let value = self.root(id)?.raw_get(&bytes(f.get(2).ok_or("key")?)?);
                if let Some(value) = value {
                    writeln!(out, "raw {}", self.value(&value)?)?;
                } else {
                    writeln!(out, "raw absent")?;
                }
            }
            "array-get" | "array-set" | "array-length" => {
                let array = self.handle(number(f, 2)?)?.clone();
                match op {
                    "array-length" => writeln!(out, "length {}", array.len())?,
                    "array-get" => {
                        if let Some(value) = array.get(f.get(3).ok_or("index")?.parse()?) {
                            writeln!(out, "get {}", self.value(&value)?)?;
                        } else {
                            writeln!(out, "host_error MissingIndex")?;
                        }
                    }
                    "array-set" => {
                        let value = self.scalar(id, f, 4)?;
                        match array.set(f.get(3).ok_or("index")?.parse()?, value) {
                            Ok(()) => writeln!(out, "set")?,
                            Err(error) => {
                                assert_eq!(error, VmError::MissingIndex);
                                writeln!(out, "host_error MissingIndex")?;
                            }
                        }
                    }
                    _ => return Err("array op".into()),
                }
            }
            _ => return Err("value command".into()),
        }
        Ok(())
    }
    fn command(&mut self, line: &str, base: &Path, out: &mut String) -> Result {
        let f: Vec<_> = line.split_whitespace().collect();
        let op = *f.first().ok_or("op")?;
        let id = number(&f, 1)?;
        match op {
            "compile" | "run" | "advance" | "drop" | "clear-result" => {
                self.program_command(op, id, &f, base, out)?;
            }
            "make-array" | "hold-result" | "hold-root" | "drop-handle" | "seed"
            | "root-store-array" | "raw" | "array-get" | "array-set" | "array-length" => {
                self.value_command(op, id, &f, out)?;
            }
            "same-object" => writeln!(
                out,
                "same {}",
                u8::from(
                    self.handle(number(&f, 2)?)?
                        .same_identity(self.handle(number(&f, 3)?)?)
                )
            )?,
            "weak-state" => {
                let label = number(&f, 2)?;
                let value = self
                    .labels
                    .get(label.checked_sub(1).ok_or("zero label")?)
                    .ok_or("label")?();
                if let Some(array) = value {
                    writeln!(out, "weak {label} {}", self.value(&Value::Array(array))?)?;
                } else {
                    writeln!(out, "weak {label} null")?;
                }
            }
            "temp" => self.temp(id, out)?,
            "refs" => writeln!(
                out,
                "string_refs {}",
                self.root(id)?
                    .realm
                    .owners(&bytes(f.get(2).ok_or("bytes")?)?)
            )?,
            "empty" => {
                let root = self.root(id)?.realm.empty_root();
                self.idle(id)?.replace_root(root)?;
                *self.ids.get_mut(id).ok_or("id")? = self.next_id;
                writeln!(out, "root {}", self.next_id)?;
                self.next_id = self.next_id.saturating_add(1);
            }
            "share" => {
                let from = number(&f, 2)?;
                let root = self.root(from)?.clone();
                self.idle(id)?.replace_root(root)?;
                let identity = *self.ids.get(from).ok_or("id")?;
                *self.ids.get_mut(id).ok_or("id")? = identity;
                writeln!(out, "root {identity}")?;
            }
            "release-child" => {
                assert_eq!(id, 1);
                *self.runners.get_mut(id).ok_or("child")? = None;
                writeln!(out, "child_released")?;
            }
            "collect" => {
                assert!(
                    self.labels.iter().all(|probe| probe().is_none()),
                    "no collector claim for live or cyclic arrays"
                );
            }
            _ => return Err(format!("unimplemented host op {op}").into()),
        }
        self.snapshot(out)
    }
}
fn native_projection(text: &str) -> Result<String> {
    let mut out = String::new();
    for line in text.lines() {
        if line.starts_with("collected ") {
            assert_eq!(
                line, "collected 0",
                "only an observed no-op collection is projected away"
            );
            continue;
        }
        if line.starts_with("stack_top ")
            || line.starts_with("array_refs ")
            || line.starts_with("external_")
            || line.starts_with("vm_root ")
        {
            continue;
        }
        if line.starts_with("compile_error ") {
            writeln!(out, "compile_error")?;
        } else if line.starts_with("runtime_error ") {
            let f: Vec<_> = line.split_whitespace().collect();
            let message = bytes(f.last().ok_or("error bytes")?)?;
            let error = if message.starts_with(b"the index ") {
                VmError::MissingIndex
            } else if message == b"division by zero" {
                VmError::DivisionByZero
            } else if message == b"indexing array with integer" || message == b"cannot iterate null"
            {
                VmError::OperandType
            } else {
                return Err(format!("unclassified native error {message:?}").into());
            };
            writeln!(out, "runtime_error {} {error:?}", f.get(1).ok_or("debt")?)?;
        } else if line.starts_with("host_error ") {
            let f: Vec<_> = line.split_whitespace().collect();
            let message = bytes(f.last().ok_or("host bytes")?)?;
            assert!(message.starts_with(b"the index "));
            writeln!(out, "host_error MissingIndex")?;
        } else {
            writeln!(out, "{line}")?;
        }
    }
    Ok(out)
}
fn corpus() -> std::path::PathBuf {
    std::env::var_os("OTTD_SCRIPT_ARRAY_CORPUS").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/arrays-native"),
        std::path::PathBuf::from,
    )
}
fn replay(base: &Path, name: &str) -> Result {
    let input = fs::read_to_string(base.join("fixtures").join(format!("{name}.session")))?;
    let native = fs::read_to_string(base.join("native").join(format!("{name}.txt")))?;
    let mut out = String::from("configured_empty_roots 0 0 1\n");
    let mut session = Session::new();
    for (step, line) in input
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .enumerate()
    {
        writeln!(out, "step {step} {line}")?;
        session.command(line, base, &mut out)?;
    }
    assert_eq!(out, native_projection(&native)?, "{name}");
    Ok(())
}
#[test]
fn actual_native_owned_array_sessions() -> Result {
    let base = corpus();
    for feed in ["buffer", "unsigned", "utf8"] {
        for family in [
            "candidate-scores",
            "rolling-history",
            "persistent-alias",
            "empty-truth-type",
            "fresh-allocation",
            "identity",
            "literal-folding",
            "indexed-alias-order",
            "defined-float-index",
            "after-write-failure",
            "host-interleave",
            "temporary-only",
            "newslot-array-error",
            "float-cast-boundary",
            "legacy-boundary-promotion",
            "host-construction",
            "shared-independent",
            "child-temporary",
            "index-error-order",
            "string-bytes",
            "malformed-array",
            "lazy-array",
            "partial-initialization",
        ] {
            replay(&base.join("attempt-01"), &format!("{family}-{feed}"))?;
        }
        for family in ["all-construction-boundaries", "set-before-after"] {
            replay(&base.join("attempt-02"), &format!("{family}-{feed}"))?;
        }
        replay(
            &base.join("attempt-03"),
            &format!("newslot-persistence-{feed}"),
        )?;
    }
    Ok(())
}

#[test]
fn native_valid_object_boundaries_keep_exact_compilation() -> Result {
    let base = corpus().join("attempt-01");
    for feed in ["buffer", "unsigned", "utf8"] {
        for (family, bodies) in [
            (
                "nested-and-cycle",
                vec![
                    ("nested-and-cycle-0", VmError::UnsupportedArrayElement),
                    ("nested-and-cycle-1", VmError::UnsupportedArrayElement),
                ],
            ),
            (
                "cycle-collection",
                vec![("native-cycle", VmError::UnsupportedArrayElement)],
            ),
            (
                "ordering-conversion",
                vec![
                    ("ordering-conversion-0", VmError::UnsupportedArrayOrdering),
                    ("ordering-conversion-1", VmError::UnsupportedRuntimeValue),
                ],
            ),
            (
                "delegate-and-missing",
                vec![
                    ("delegate-and-missing-0", VmError::UnsupportedRuntimeValue),
                    ("delegate-and-missing-1", VmError::MissingIndex),
                    ("delegate-and-missing-2", VmError::MissingIndex),
                ],
            ),
        ] {
            let native =
                fs::read_to_string(base.join("native").join(format!("{family}-{feed}.txt")))?;
            let blocks: Vec<_> = native
                .split("compiled\n")
                .skip(1)
                .map(|part| {
                    part.lines()
                        .take_while(|line| {
                            line.starts_with("stack ")
                                || line.starts_with("literal ")
                                || line.starts_with("op ")
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                        + "\n"
                })
                .collect();
            let mut bodies: Vec<_> = bodies
                .into_iter()
                .map(|(name, error)| (name, Err(error)))
                .collect();
            if family == "nested-and-cycle" {
                bodies.push(("scalar-return", Ok(Execution::Returned(Value::Integer(0)))));
            }
            assert_eq!(blocks.len(), bodies.len());
            for ((name, expected), block) in bodies.into_iter().zip(blocks) {
                let source = fs::read(base.join("fixtures").join(format!("{name}.nut")))?;
                let source = if feed == "unsigned" {
                    source
                        .into_iter()
                        .map(char::from)
                        .collect::<String>()
                        .into_bytes()
                } else {
                    source
                };
                let program = crate::compile_bytes(&source)?;
                let mut compiled = String::new();
                dump(&program, &mut compiled)?;
                assert_eq!(compiled, block, "{family}-{feed}/{name}");
                assert_eq!(Vm::new(&program)?.resume(10_000), expected, "{name}");
            }
        }
    }
    Ok(())
}
