use super::Lexer;
use crate::{CompileError, CompileErrorKind, NativeCharacterContext, Value};
impl Lexer<'_> {
    pub(super) fn number(&mut self) -> Result<Value, CompileError> {
        let start = self.position;
        let first = self.peek();
        self.advance()?;
        let radix = if first == Some(b'0') {
            match self.classify(NativeCharacterContext::Number)? {
                Some(b'x' | b'X') => {
                    self.advance()?;
                    Some(16)
                }
                Some(b'0'..=b'7') => Some(8),
                _ => None,
            }
        } else {
            None
        };
        if let Some(radix) = radix {
            let digits = self.position;
            while self
                .classify(NativeCharacterContext::Number)?
                .is_some_and(|b| {
                    if radix == 16 {
                        b.is_ascii_hexdigit()
                    } else {
                        (b'0'..=b'7').contains(&b)
                    }
                })
            {
                self.advance()?;
            }
            if (radix == 8
                && self
                    .classify(NativeCharacterContext::Number)?
                    .is_some_and(|b| b.is_ascii_digit()))
                || (radix == 16 && self.position.saturating_sub(digits) > 16)
            {
                return Err(self.error(CompileErrorKind::InvalidNumber));
            }
            let text = self
                .source
                .get(digits..self.position)
                .ok_or_else(|| self.error(CompileErrorKind::InvalidNumber))?;
            let text = std::str::from_utf8(text)
                .map_err(|_| self.error(CompileErrorKind::InvalidCharacter))?;
            return Ok(integer(text, radix));
        }
        let mut float = false;
        while let Some(c) = self.classify(NativeCharacterContext::Number)? {
            match c {
                b'0'..=b'9' => self.advance()?,
                b'.' => {
                    float = true;
                    self.advance()?;
                }
                b'e' | b'E' => {
                    float = true;
                    self.advance()?;
                    if matches!(self.peek(), Some(b'+' | b'-')) {
                        self.advance()?;
                    }
                    if !self
                        .classify(NativeCharacterContext::Number)?
                        .is_some_and(|b| b.is_ascii_digit())
                    {
                        return Err(self.error(CompileErrorKind::InvalidNumber));
                    }
                }
                _ => break,
            }
        }
        let text = self
            .source
            .get(start..self.position)
            .ok_or_else(|| self.error(CompileErrorKind::InvalidNumber))?;
        let text = std::str::from_utf8(text)
            .map_err(|_| self.error(CompileErrorKind::InvalidCharacter))?;
        if float {
            parse_float(text)
                .map(Value::Float)
                .ok_or_else(|| self.error(CompileErrorKind::InvalidNumber))
        } else {
            Ok(integer(text, 10))
        }
    }
}
fn integer(text: &str, radix: u32) -> Value {
    let unsigned = u64::from_str_radix(text, radix).unwrap_or(0);
    Value::Integer(i64::from_ne_bytes(unsigned.to_ne_bytes()))
}
#[expect(
    clippy::cast_possible_truncation,
    reason = "Native lexing uses strtod followed by an explicit SQFloat=f32 cast"
)]
fn parse_float(text: &str) -> Option<u32> {
    text.parse::<f64>().ok().map(|n| (n as f32).to_bits())
}
