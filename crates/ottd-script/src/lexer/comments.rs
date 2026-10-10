//! Native trivia preserves the distinction between block LF and newline tokens.
use super::{CompileError, CompileErrorKind, Lexer};
impl Lexer<'_> {
    fn following(&self) -> Option<u8> {
        self.source
            .as_bytes()
            .get(self.position.saturating_add(1))
            .copied()
    }
    pub(super) fn trivia(&mut self) -> Result<bool, CompileError> {
        let mut newline = false;
        loop {
            match (self.peek(), self.following()) {
                (Some(b' ' | b'\t' | b'\r'), _) => self.advance(),
                (Some(b'\n'), _) => {
                    newline = true;
                    self.advance();
                }
                (Some(b'/'), Some(b'/')) => {
                    self.advance();
                    self.advance();
                    while !matches!(self.peek(), None | Some(b'\n' | 0)) {
                        self.advance();
                    }
                    if self.peek() == Some(0) {
                        self.position = self.source.len();
                    }
                }
                (Some(b'/'), Some(b'*')) => {
                    self.advance();
                    self.advance();
                    self.block_comment()?;
                }
                _ => return Ok(newline),
            }
        }
    }
    fn block_comment(&mut self) -> Result<(), CompileError> {
        loop {
            match (self.peek(), self.following()) {
                (None | Some(0), _) => return Err(self.error(CompileErrorKind::ExpectedToken)),
                (Some(b'*'), Some(b'/')) => {
                    self.advance();
                    self.advance();
                    return Ok(());
                }
                _ => self.advance(),
            }
        }
    }
}
