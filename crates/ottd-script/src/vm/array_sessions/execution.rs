//! Compile and resume the production frame from array host commands.
use super::{Result, state::Session};
use crate::vm::root_sessions::{dump, number};
use crate::vm::{Execution, Storage, Vm};
use crate::{CompileErrorKind, Value, VmError};
use std::{fmt::Write, fs, path::Path};

impl Session {
    pub(super) fn execute(&mut self, id: usize, credit: u32, out: &mut String) -> Result {
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
                        Value::Table(root) if root.same_identity(&self.root(id)?.table) => {
                            writeln!(
                                out,
                                "frame {index} root {}",
                                self.ids.get(id).ok_or("root id")?
                            )?;
                        }
                        value => writeln!(out, "frame {index} {}", self.value(value)?)?,
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
    pub(super) fn program_command(
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
}
