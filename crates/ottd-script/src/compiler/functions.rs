//! Child prototypes share the lexer while retaining independent register state.
use super::{Compiler, Emitter, Register, Registers};
use crate::compiler::diagnostics::CompileFailure;
use crate::{
    CompileErrorKind, Instruction, Value,
    lexer::TokenKind,
    prototype::{Capture, Data},
    table::scalar::Scalar,
};
use std::{mem, rc::Rc};
struct Parameters<'a> {
    registers: Registers<'a>,
    defaults: Vec<u8>,
    names: Vec<crate::ByteString>,
    variadic: bool,
}
impl<'a> Compiler<'a> {
    pub(super) fn scalar_literals(&self) -> Result<Vec<Scalar>, CompileFailure> {
        self.literals
            .iter()
            .cloned()
            .map(|value| {
                Scalar::from_value(value, &self.realm)
                    .map_err(|_| self.error(CompileErrorKind::UnsupportedSyntax))
            })
            .collect()
    }
    pub(super) fn function_statement(&mut self, depth: u8) -> Result<(), CompileFailure> {
        self.advance()?;
        let TokenKind::Identifier(name) = self.token.kind else {
            return Err(self.expected_identifier());
        };
        self.advance()?;
        self.registers.reference(Register(0));
        let key = self.push()?;
        self.load(key, Value::String(self.realm.string(name.as_bytes())))?;
        self.expect(b'(')?;
        self.create_function(depth, Some(name))?;
        let value = self.pop()?;
        let key = self.pop()?;
        let receiver = self.pop()?;
        let target = self.push()?;
        self.emit(Instruction {
            opcode: 0x0b,
            arg0: target.0,
            arg1: i32::from(receiver.0),
            arg2: key.0,
            arg3: value.0,
        });
        let _target = self.pop()?;
        Ok(())
    }
    pub(super) fn create_function(
        &mut self,
        depth: u8,
        name: Option<&str>,
    ) -> Result<(), CompileFailure> {
        self.depth(depth)?;
        let parameters = self.parameters(depth)?;
        let mut captures = Vec::new();
        if self.token.kind == TokenKind::Symbol(b':') {
            self.advance()?;
            self.expect(b'(')?;
            while self.token.kind != TokenKind::Symbol(b')') {
                let TokenKind::Identifier(name) = self.token.kind else {
                    return Err(self.expected_identifier());
                };
                let capture = if let Some(register) = self.registers.local(name) {
                    Capture::Local(register.0)
                } else if let Some(index) =
                    self.captures.iter().position(|(found, _)| *found == name)
                {
                    Capture::Outer(
                        u8::try_from(index).map_err(|_| self.error(CompileErrorKind::Limit))?,
                    )
                } else {
                    Capture::Symbol(self.realm.string(name.as_bytes()))
                };
                captures.push((name, capture));
                self.advance()?;
                if self.token.kind == TokenKind::Symbol(b',') {
                    self.advance()?;
                } else if self.token.kind != TokenKind::Symbol(b')') {
                    return Err(self
                        .error(CompileErrorKind::ExpectedToken)
                        .native("expected ')' or ','"));
                }
            }
            self.advance()?;
        }
        let parent_registers = mem::replace(&mut self.registers, parameters.registers);
        let parent_emitter = mem::replace(&mut self.emitter, Emitter::new());
        let parent_literals = mem::take(&mut self.literals);
        let parent_children = mem::take(&mut self.children);
        let parent_captures = mem::replace(&mut self.captures, captures);
        let parent_breaks = mem::take(&mut self.break_targets);
        let parent_continues = mem::take(&mut self.continue_targets);
        let parent_last_stack = mem::replace(&mut self.last_stack_size, 0);
        self.function_depth = self
            .function_depth
            .checked_add(1)
            .ok_or_else(|| self.error(CompileErrorKind::Limit))?;
        self.statement(depth)?;
        self.emit(Instruction {
            opcode: 0x13,
            arg0: 255,
            arg1: 0,
            arg2: 0,
            arg3: 0,
        });
        self.registers.truncate(0);
        let literals = self.scalar_literals()?;
        let prototype = Data {
            source_name: Some(self.source_name.clone()),
            name: name.map(|name| self.realm.string(name.as_bytes())),
            parameters: parameters.names.len(),
            variadic: parameters.variadic,
            parameter_names: parameters.names,
            capture_names: self
                .captures
                .iter()
                .map(|(name, _)| self.realm.string(name.as_bytes()))
                .collect(),
            local_names: self
                .registers
                .local_names
                .iter()
                .map(|name| self.realm.string(name.as_bytes()))
                .collect(),
            stack_size: self.registers.high_water,
            literals,
            instructions: mem::replace(&mut self.emitter, parent_emitter).instructions,
            children: mem::replace(&mut self.children, parent_children),
            captures: mem::replace(&mut self.captures, parent_captures)
                .into_iter()
                .map(|(_, capture)| capture)
                .collect(),
            defaults: parameters.defaults,
        };
        self.registers = parent_registers;
        self.literals = parent_literals;
        self.break_targets = parent_breaks;
        self.continue_targets = parent_continues;
        self.last_stack_size = parent_last_stack;
        self.function_depth = self.function_depth.saturating_sub(1);
        let index =
            i32::try_from(self.children.len()).map_err(|_| self.error(CompileErrorKind::Limit))?;
        self.children.push(Rc::new(prototype));
        let target = self.push()?;
        self.emit(Instruction {
            opcode: 0x30,
            arg0: target.0,
            arg1: index,
            arg2: 0,
            arg3: 0,
        });
        Ok(())
    }
    fn parameters(&mut self, depth: u8) -> Result<Parameters<'a>, CompileFailure> {
        let mut registers = Registers::new();
        let mut defaults = Vec::new();
        let mut parameters = vec![self.realm.string(b"this")];
        let mut variadic = false;
        while self.token.kind != TokenKind::Symbol(b')') {
            if self.token.kind == TokenKind::Ellipsis {
                if !defaults.is_empty() {
                    return Err(self.error(CompileErrorKind::ExpectedToken).native("function with default parameters cannot have variable number of parameters"));
                }
                variadic = true;
                self.advance()?;
                if self.token.kind != TokenKind::Symbol(b')') {
                    return Err(self
                        .error(CompileErrorKind::ExpectedToken)
                        .native("expected ')'"));
                }
                break;
            }
            let TokenKind::Identifier(name) = self.token.kind else {
                return Err(self.expected_identifier());
            };
            registers
                .bind(name)
                .ok_or_else(|| self.error(CompileErrorKind::Limit))?;
            parameters.push(self.realm.string(name.as_bytes()));
            self.advance()?;
            if self.token.kind == TokenKind::Symbol(b'=') {
                self.advance()?;
                self.expression(depth)?;
                defaults.push(
                    self.registers
                        .top()
                        .ok_or_else(|| self.error(CompileErrorKind::ExpectedToken))?
                        .0,
                );
            } else if !defaults.is_empty() {
                return Err(self
                    .error(CompileErrorKind::ExpectedToken)
                    .native("expected '='"));
            }
            if self.token.kind == TokenKind::Symbol(b',') {
                self.advance()?;
            } else if self.token.kind != TokenKind::Symbol(b')') {
                return Err(self
                    .error(CompileErrorKind::ExpectedToken)
                    .native("expected ')' or ','"));
            }
        }
        self.advance()?;
        for _ in &defaults {
            self.pop()?;
        }
        Ok(Parameters {
            registers,
            defaults,
            names: parameters,
            variadic,
        })
    }
}
