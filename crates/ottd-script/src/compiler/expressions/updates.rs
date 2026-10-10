//! Assignment and conditional-expression lowering with native state boundaries.
use super::{Compiler, ExpressionState};
use crate::{CompileError, CompileErrorKind, Instruction, lexer::TokenKind};
impl Compiler<'_> {
    pub(super) fn assignment(
        &mut self,
        depth: u8,
        state: &ExpressionState,
    ) -> Result<(), CompileError> {
        if state.dereference.is_none() {
            return Err(self.error(CompileErrorKind::UnsupportedSyntax));
        }
        let operation = self.token.kind.clone();
        self.advance()?;
        let _rhs = self.expression(self.depth(depth)?)?;
        let right = self.pop()?;
        if let TokenKind::Compound(operator) = operation {
            let left = self.pop()?;
            let target = self.push()?;
            self.emit(Instruction {
                opcode: 0x23,
                arg0: target.0,
                arg1: i32::from(left.0),
                arg2: right.0,
                arg3: operator,
            });
        } else {
            let target = self
                .registers
                .top()
                .ok_or_else(|| self.error(CompileErrorKind::ExpectedToken))?;
            self.move_to(target, right);
        }
        Ok(())
    }
    pub(super) fn ternary(&mut self, depth: u8) -> Result<(), CompileError> {
        self.advance()?;
        let condition = self.pop()?;
        self.emit(Instruction {
            opcode: 0x1a,
            arg0: condition.0,
            arg1: 0,
            arg2: 0,
            arg3: 0,
        });
        let conditional = self.position()?;
        let target = self.push()?;
        let _first = self.expression(self.depth(depth)?)?;
        let first = self.pop()?;
        if target != first {
            self.move_to(target, first);
        }
        let end_first = self.position()?;
        self.emit(Instruction {
            opcode: 0x18,
            arg0: 0,
            arg1: 0,
            arg2: 0,
            arg3: 0,
        });
        self.expect(b':')?;
        let skip = self.position()?;
        let _second = self.expression(self.depth(depth)?)?;
        let second = self.pop()?;
        if target != second {
            self.move_to(target, second);
        }
        self.patch(skip, self.position()?)?;
        self.patch(
            conditional,
            end_first
                .checked_add(1)
                .ok_or_else(|| self.error(CompileErrorKind::Limit))?,
        )?;
        self.emitter.barrier();
        Ok(())
    }
}
