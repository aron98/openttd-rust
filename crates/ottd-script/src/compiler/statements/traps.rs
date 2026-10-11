//! Protected statements bind catch locals after patching native trap control flow.
use super::super::Compiler;
use crate::{
    CompileErrorKind, Instruction, compiler::diagnostics::CompileFailure, lexer::TokenKind,
};

impl Compiler<'_> {
    fn trap_nesting(&mut self, entering: bool) -> Result<(), CompileFailure> {
        let offset = self.token.offset;
        for count in std::iter::once(&mut self.traps)
            .chain(
                self.break_targets
                    .last_mut()
                    .map(|target| &mut target.traps),
            )
            .chain(
                self.continue_targets
                    .last_mut()
                    .map(|target| &mut target.traps),
            )
        {
            *count = if entering {
                count.checked_add(1)
            } else {
                count.checked_sub(1)
            }
            .ok_or_else(|| CompileFailure::new(offset, CompileErrorKind::Limit))?;
        }
        Ok(())
    }
    pub(super) fn pop_traps(&mut self, count: u8) {
        if count != 0 {
            self.emit(Instruction {
                opcode: 0x39,
                arg0: count,
                arg1: 0,
                arg2: 0,
                arg3: 0,
            });
        }
    }
    pub(super) fn try_catch(&mut self, depth: u8) -> Result<(), CompileFailure> {
        self.advance()?;
        self.emit(Instruction {
            opcode: 0x38,
            arg0: 0,
            arg1: 0,
            arg2: 0,
            arg3: 0,
        });
        self.trap_nesting(true)?;
        let trap = self.position()?;
        self.statement(depth)?;
        self.trap_nesting(false)?;
        self.pop_traps(1);
        self.emit(Instruction {
            opcode: 0x18,
            arg0: 0,
            arg1: 0,
            arg2: 0,
            arg3: 0,
        });
        let skip = self.position()?;
        self.patch(trap, skip)?;
        if self.token.kind != TokenKind::Catch {
            return Err(self
                .error(CompileErrorKind::ExpectedToken)
                .native("expected 'catch'"));
        }
        self.advance()?;
        self.expect(b'(')?;
        let TokenKind::Identifier(name) = self.token.kind else {
            return Err(self.expected_identifier());
        };
        self.advance()?;
        self.expect(b')')?;
        let size = self.size()?;
        let target = self
            .registers
            .bind(name)
            .ok_or_else(|| self.error(CompileErrorKind::Limit))?;
        let index = usize::try_from(trap).map_err(|_| self.error(CompileErrorKind::Limit))?;
        let error = self.error(CompileErrorKind::Limit);
        self.emitter.instructions.get_mut(index).ok_or(error)?.arg0 = target.0;
        self.statement(depth)?;
        self.patch(skip, self.position()?)?;
        self.registers.truncate(size);
        Ok(())
    }
    pub(super) fn throw_statement(&mut self) -> Result<(), CompileFailure> {
        self.advance()?;
        self.comma(0)?;
        let target = self.pop()?;
        self.emit(Instruction {
            opcode: 0x3a,
            arg0: target.0,
            arg1: 0,
            arg2: 0,
            arg3: 0,
        });
        Ok(())
    }
}
