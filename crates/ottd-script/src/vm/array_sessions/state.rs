//! Root, handle and weak identity state for native array sessions.
use super::Result;
use crate::vm::root_sessions::{bytes, render};
use crate::vm::{Temporary, Vm};
use crate::{Array, Program, Realm, RootEnvironment, Runner, Value};
use std::fmt::Write;

pub(super) struct Session {
    pub(super) runners: Vec<Option<Runner>>,
    pub(super) frames: Vec<Option<Vm<'static>>>,
    pub(super) programs: Vec<Option<(usize, Program)>>,
    pub(super) results: Vec<Option<Value>>,
    pub(super) handles: Vec<Option<Array>>,
    pub(super) labels: Vec<Box<dyn Fn() -> Option<Array>>>,
    pub(super) ids: Vec<usize>,
    pub(super) next_id: usize,
}
impl Session {
    pub(super) fn new() -> Self {
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
    pub(super) fn runner(&self, id: usize) -> Result<&Runner> {
        if let Some(frame) = self.frames.get(id).ok_or("frame index")? {
            return Ok(frame.runner.get());
        }
        self.runners
            .get(id)
            .and_then(Option::as_ref)
            .ok_or_else(|| "missing runner".into())
    }
    pub(super) fn idle(&mut self, id: usize) -> Result<&mut Runner> {
        self.runners
            .get_mut(id)
            .and_then(Option::as_mut)
            .ok_or_else(|| "runner busy/released".into())
    }
    pub(super) fn root(&self, id: usize) -> Result<&RootEnvironment> {
        Ok(&self.runner(id)?.root)
    }
    pub(super) fn handle(&self, slot: usize) -> Result<&Array> {
        self.handles
            .get(slot)
            .and_then(Option::as_ref)
            .ok_or_else(|| "missing array handle".into())
    }
    pub(super) fn label(&mut self, array: &Array) -> usize {
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
    pub(super) fn value(&mut self, value: &Value) -> Result<String> {
        match value {
            Value::Array(array) => Ok(format!(
                "array {} length {}",
                self.label(array),
                array.len()
            )),
            Value::NativeClosure(_)
            | Value::ScriptClosure(_)
            | Value::Table(_)
            | Value::Null
            | Value::Bool(_)
            | Value::Integer(_)
            | Value::Float(_)
            | Value::String(_) => render(value),
        }
    }
    pub(super) fn temp(&mut self, id: usize, out: &mut String) -> Result {
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
    pub(super) fn snapshot(&mut self, out: &mut String) -> Result {
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
    pub(super) fn scalar(&self, id: usize, fields: &[&str], kind: usize) -> Result<Value> {
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
}
