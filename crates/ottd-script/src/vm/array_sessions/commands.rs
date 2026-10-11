//! Array ownership and shared-root operations in the passive host protocol.
use super::{Result, state::Session};
use crate::vm::root_sessions::{bytes, number};
use crate::{Value, VmError};
use std::{fmt::Write, path::Path};

impl Session {
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
    pub(super) fn command(&mut self, line: &str, base: &Path, out: &mut String) -> Result {
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
