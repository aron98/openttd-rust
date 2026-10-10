//! Pinned `DecodeUtf8` plus `SQLexer::Next` admission, evaluated one lookahead at a time.
use super::{CompileError, CompileErrorKind, Lexer};
impl Lexer<'_> {
    pub(super) fn read(&mut self) -> Result<(), CompileError> {
        let tail = self
            .source
            .get(self.position..)
            .ok_or_else(|| self.error(CompileErrorKind::InvalidCharacter))?;
        if tail.is_empty() {
            self.width = 0;
            return Ok(());
        }
        let (character, width) =
            decode(tail).ok_or_else(|| self.error(CompileErrorKind::InvalidCharacter))?;
        if character > 0xffff {
            return Err(self.error(CompileErrorKind::InvalidCharacter));
        }
        self.width = if character == 0 { 0 } else { width };
        Ok(())
    }
}
fn decode(bytes: &[u8]) -> Option<(u32, usize)> {
    let first = *bytes.first()?;
    if first < 0x80 {
        return Some((u32::from(first), 1));
    }
    let (width, mask, minimum) = match first {
        0xc0..=0xdf => (2, 0x1f, 0x80),
        0xe0..=0xef => (3, 0x0f, 0x800),
        0xf0..=0xf7 => (4, 0x07, 0x10000),
        _ => return None,
    };
    let mut character = u32::from(first & mask);
    for byte in bytes.get(1..width)? {
        if byte & 0xc0 != 0x80 {
            return None;
        }
        character = (character << 6) | u32::from(byte & 0x3f);
    }
    // Native DecodeUtf8 accepts encoded surrogate codepoints, unlike Rust str.
    (character >= minimum && character <= 0x10_ffff).then_some((character, width))
}
