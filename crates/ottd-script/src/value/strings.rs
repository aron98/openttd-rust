//! Bundled fmt's shortest f32 digits with its general-format exponent thresholds.
use crate::{Realm, Value, VmError};
impl Value {
    pub(crate) fn concatenate(&self, other: &Self, realm: &Realm) -> Result<Self, VmError> {
        let mut bytes = self.text()?;
        bytes.extend(other.text()?);
        Ok(Self::String(realm.string(&bytes)))
    }
    fn text(&self) -> Result<Vec<u8>, VmError> {
        Ok(match self {
            Self::Array(_) => return Err(VmError::UnsupportedRuntimeValue),
            Self::String(value) => value.as_bytes().to_vec(),
            Self::Integer(value) => value.to_string().into_bytes(),
            Self::Bool(value) => value.to_string().into_bytes(),
            Self::Null => b"(null : 0x00000000)".to_vec(),
            Self::Float(bits) => float_text(*bits).into_bytes(),
        })
    }
    pub(crate) fn type_name(&self, realm: &Realm) -> Self {
        let name: &[u8] = match self {
            Self::String(_) => b"string",
            Self::Array(_) => b"array",
            Self::Integer(_) => b"integer",
            Self::Bool(_) => b"bool",
            Self::Null => b"null",
            Self::Float(_) => b"float",
        };
        Self::String(realm.string(name))
    }
}
#[expect(
    clippy::arithmetic_side_effects,
    reason = "Ryu finite f32 output has at most 9 significant digits and exponent -45..38; all digit/decimal-position arithmetic fits i16"
)]
fn float_text(bits: u32) -> String {
    let value = f32::from_bits(bits);
    let mut output = String::new();
    if value.is_sign_negative() {
        output.push('-');
    }
    if value.is_nan() {
        output.push_str("nan");
        return output;
    }
    if value.is_infinite() {
        output.push_str("inf");
        return output;
    }
    let mut buffer = ryu::Buffer::new();
    let shortest = buffer.format_finite(value.abs());
    let (mantissa, exponent) = shortest.split_once('e').unwrap_or((shortest, "0"));
    // Ryu's finite f32 exponent has at most two digits; all arithmetic fits i16.
    let mut power = exponent
        .bytes()
        .filter(u8::is_ascii_digit)
        .fold(0_i16, |value, digit| value * 10 + i16::from(digit - b'0'));
    if exponent.starts_with('-') {
        power = -power;
    }
    let mut digits = String::new();
    let mut count = 0_i16;
    let mut fractional = false;
    for byte in mantissa.bytes() {
        if byte == b'.' {
            fractional = true;
        } else {
            digits.push(char::from(byte));
            count += 1;
            if fractional {
                power -= 1;
            }
        }
    }
    while count > 1 && digits.ends_with('0') {
        digits.pop();
        count -= 1;
        power += 1;
    }
    while count > 1 && digits.starts_with('0') {
        digits.remove(0);
        count -= 1;
    }
    let exponent = power + count - 1;
    if (-4..7).contains(&exponent) {
        let point = power + count;
        if point <= 0 {
            output.push_str("0.");
            for _ in 0..-point {
                output.push('0');
            }
            output.push_str(&digits);
        } else {
            for (index, character) in (0_i16..).zip(digits.chars()) {
                if index == point {
                    output.push('.');
                }
                output.push(character);
            }
            for _ in count..point {
                output.push('0');
            }
        }
    } else {
        let mut characters = digits.chars();
        output.extend(characters.next());
        if count > 1 {
            output.push('.');
            output.extend(characters);
        }
        output.push('e');
        output.push(if exponent < 0 { '-' } else { '+' });
        let magnitude = exponent.unsigned_abs();
        if magnitude < 10 {
            output.push('0');
        }
        output.push_str(&magnitude.to_string());
    }
    output
}
