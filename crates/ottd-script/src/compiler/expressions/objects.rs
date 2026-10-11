//! Native delete owns a nested expression state and retains its terminal field.
use super::{Compiler, ExpressionState};
use crate::{CompileError, CompileErrorKind, Instruction};
impl Compiler<'_> {
    pub(super) fn unary_expression(
        &mut self,
        depth: u8,
        state: &mut ExpressionState,
        opcode: u8,
    ) -> Result<(), CompileError> {
        self.advance()?;
        self.prefixed(depth, state)?;
        let source = self.pop()?;
        let target = self.push()?;
        self.emit(Instruction {
            opcode,
            arg0: target.0,
            arg1: i32::from(source.0),
            arg2: 0,
            arg3: 0,
        });
        Ok(())
    }
    pub(super) fn delete_expression(&mut self, depth: u8) -> Result<(), CompileError> {
        self.advance()?;
        let mut state = ExpressionState {
            deleting: true,
            ..ExpressionState::default()
        };
        self.prefixed(depth, &mut state)?;
        if !state.field {
            return Err(self.error(CompileErrorKind::ExpectedToken));
        }
        let key = self.pop()?;
        let receiver = self.pop()?;
        let target = self.push()?;
        self.emit(Instruction {
            opcode: 0x0c,
            arg0: target.0,
            arg1: i32::from(receiver.0),
            arg2: key.0,
            arg3: 0,
        });
        Ok(())
    }
}
