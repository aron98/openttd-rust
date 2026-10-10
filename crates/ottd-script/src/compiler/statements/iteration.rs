//! Native post-test loops and for-increment extraction/re-emission.
use super::super::Compiler;
use crate::{CompileError, CompileErrorKind, Instruction, lexer::TokenKind};
impl Compiler<'_> {
    pub(super) fn begin_loop(&mut self) -> Result<(), CompileError> {
        self.break_targets.push(Vec::new());
        self.continue_targets.push(Vec::new());
        self.last_stack_size = self.size()?;
        Ok(())
    }
    pub(super) fn finish_breaks(&mut self) -> Result<(), CompileError> {
        let targets = self
            .break_targets
            .pop()
            .ok_or_else(|| self.error(CompileErrorKind::ExpectedToken))?;
        let end = self.position()?;
        for position in targets {
            self.patch(position, end)?;
        }
        Ok(())
    }
    pub(super) fn finish_loop(&mut self, continue_target: i32) -> Result<(), CompileError> {
        self.finish_breaks()?;
        let targets = self
            .continue_targets
            .pop()
            .ok_or_else(|| self.error(CompileErrorKind::ExpectedToken))?;
        for position in targets {
            self.patch(position, continue_target)?;
        }
        Ok(())
    }
    pub(super) fn do_loop(&mut self, depth: u8) -> Result<(), CompileError> {
        self.advance()?;
        let head = self.position()?;
        let size = self.size()?;
        self.begin_loop()?;
        self.statement(depth)?;
        self.registers.truncate(size);
        if self.token.kind != TokenKind::While {
            return Err(self.error(CompileErrorKind::ExpectedToken));
        }
        self.advance()?;
        let continue_target = self.position()?;
        self.expect(b'(')?;
        self.comma(0)?;
        self.expect(b')')?;
        let condition = self.pop()?;
        self.emit(Instruction {
            opcode: 0x19,
            arg0: condition.0,
            arg1: 0,
            arg2: 0,
            arg3: 0,
        });
        self.patch(self.position()?, head)?;
        self.finish_loop(continue_target)
    }
    pub(super) fn for_loop(&mut self, depth: u8) -> Result<(), CompileError> {
        self.advance()?;
        let size = self.size()?;
        self.expect(b'(')?;
        if self.token.kind == TokenKind::Local {
            self.local()?;
        } else if self.token.kind != TokenKind::Symbol(b';') {
            self.comma(0)?;
            let _target = self.pop()?;
        }
        self.expect(b';')?;
        self.emitter.barrier();
        let head = self.position()?;
        let condition = if self.token.kind == TokenKind::Symbol(b';') {
            None
        } else {
            self.comma(0)?;
            let source = self.pop()?;
            self.emit(Instruction {
                opcode: 0x1a,
                arg0: source.0,
                arg1: 0,
                arg2: 0,
                arg3: 0,
            });
            Some(self.position()?)
        };
        self.expect(b';')?;
        self.emitter.barrier();
        let start = self.emitter.instructions.len();
        if self.token.kind != TokenKind::Symbol(b')') {
            self.comma(0)?;
            let _target = self.pop()?;
        }
        self.expect(b')')?;
        self.emitter.barrier();
        let increment = self.emitter.instructions.split_off(start);
        self.begin_loop()?;
        self.statement(depth)?;
        let continue_target = self.position()?;
        for instruction in increment {
            self.emit(instruction);
        }
        self.emit(Instruction {
            opcode: 0x18,
            arg0: 0,
            arg1: 0,
            arg2: 0,
            arg3: 0,
        });
        self.patch(self.position()?, head)?;
        if let Some(position) = condition {
            self.patch(position, self.position()?)?;
        }
        self.registers.truncate(size);
        self.finish_loop(continue_target)
    }
}
