//! String decoding follows native codepoint input and `APPEND_CHAR` UTF-8 encoding.
use super::{CompileError, CompileErrorKind, Lexer, NativeCharacterContext};
use crate::Value;
impl Lexer<'_> {
    pub(super) fn string(&mut self, verbatim: bool) -> Result<Value, CompileError> {
        let delimiter = self
            .peek()
            .ok_or_else(|| self.error(CompileErrorKind::ExpectedToken))?;
        self.advance()?;
        let mut bytes = Vec::new();
        loop {
            match self.peek() {
                None | Some(0) => return Err(self.error(CompileErrorKind::ExpectedToken)),
                Some(byte) if byte == delimiter => {
                    self.advance()?;
                    if verbatim && self.peek() == Some(b'"') {
                        bytes.push(b'"');
                        self.advance()?;
                    } else {
                        break;
                    }
                }
                Some(b'\n') if !verbatim => return Err(self.error(CompileErrorKind::ExpectedToken)),
                Some(b'\\') if !verbatim => {
                    self.advance()?;
                    self.escape(&mut bytes)?;
                }
                Some(_) => {
                    let encoded = self
                        .source
                        .get(self.position..self.position.saturating_add(self.width))
                        .ok_or_else(|| self.error(CompileErrorKind::InvalidCharacter))?;
                    bytes.extend_from_slice(encoded);
                    self.advance()?;
                }
            }
        }
        if delimiter == b'\'' {
            let [byte] = bytes.as_slice() else {
                return Err(self.error(CompileErrorKind::ExpectedToken));
            };
            return Ok(Value::Integer(i64::from(*byte)));
        }
        Ok(Value::String(self.realm.string(&bytes)))
    }
    fn escape(&mut self, bytes: &mut Vec<u8>) -> Result<(), CompileError> {
        let byte = self
            .peek()
            .ok_or_else(|| self.error(CompileErrorKind::ExpectedToken))?;
        if byte == b'x' {
            self.advance()?;
            let mut value = 0_u16;
            let mut count = 0_u8;
            while let Some(digit) = self
                .classify(NativeCharacterContext::HexEscape)?
                .and_then(|c| char::from(c).to_digit(16))
            {
                // Native checks isxdigit before the four-digit limit.
                if count == 4 {
                    break;
                }
                let digit = u16::try_from(digit)
                    .map_err(|_| self.error(CompileErrorKind::InvalidNumber))?;
                value = value.wrapping_mul(16).wrapping_add(digit);
                self.advance()?;
                count = count.saturating_add(1);
            }
            if count == 0 {
                return Err(self.error(CompileErrorKind::InvalidNumber));
            }
            // Native narrows through target C char before APPEND_CHAR encoding.
            let narrowed = std::ffi::c_char::from_ne_bytes([value.to_le_bytes()[0]]);
            if let Ok(codepoint) = u32::try_from(narrowed) {
                if let Some(character) = char::from_u32(codepoint) {
                    let mut buffer = [0; 4];
                    bytes.extend_from_slice(character.encode_utf8(&mut buffer).as_bytes());
                }
            }
            return Ok(());
        }
        bytes.push(match byte {
            b't' => b'\t',
            b'a' => 7,
            b'b' => 8,
            b'n' => b'\n',
            b'r' => b'\r',
            b'v' => 11,
            b'f' => 12,
            b'0' => 0,
            b'\\' | b'"' | b'\'' => byte,
            _ => return Err(self.error(CompileErrorKind::ExpectedToken)),
        });
        self.advance()?;
        Ok(())
    }
}
