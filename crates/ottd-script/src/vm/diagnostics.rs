//! `sqdebug.cpp` diagnostic rendering is distinct from ordinary string conversion.
use super::{Value, Vm, VmError};

impl Vm<'_> {
    pub(super) fn index_failure(&mut self, key: &Value, error: VmError) -> VmError {
        let prefix = match diagnostic_prefix(key) {
            Ok(prefix) => prefix,
            Err(error) => return error,
        };
        let mut message = b"the index '".to_vec();
        message.extend_from_slice(&prefix);
        message.extend_from_slice(b"' does not exist");
        self.native_failure(&message, error)
    }
}

pub(super) fn diagnostic_prefix(value: &Value) -> Result<Vec<u8>, VmError> {
    let rendered = match value {
        Value::String(value) => value.as_bytes().to_vec(),
        Value::Integer(value) => value.to_string().into_bytes(),
        Value::Float(bits) => match float_diagnostic(*bits) {
            Ok(value) => value.into_bytes(),
            Err(failure) => return Err(failure),
        },
        Value::Null
        | Value::Bool(_)
        | Value::Array(_)
        | Value::Table(_)
        | Value::NativeClosure(_)
        | Value::ScriptClosure(_) => value.type_label().as_bytes().to_vec(),
    };
    // Bundled fmt counts valid UTF-8 codepoints and treats each invalid byte
    // as one codepoint; SQString::View preserves embedded NUL bytes.
    let length = rendered
        .utf8_chunks()
        .flat_map(|chunk| {
            chunk
                .valid()
                .chars()
                .map(char::len_utf8)
                .chain(chunk.invalid().iter().map(|_| 1))
        })
        .take(50)
        .sum();
    let Some(prefix) = rendered.get(..length) else {
        return Err(VmError::InvalidBytecode);
    };
    Ok(prefix.to_vec())
}

fn float_diagnostic(bits: u32) -> Result<String, VmError> {
    let value = f64::from(f32::from_bits(bits));
    if !value.is_finite() {
        let name = if value.is_nan() { "nan" } else { "inf" };
        return Ok(format!(
            "{}{name}",
            if value.is_sign_negative() { "-" } else { "" }
        ));
    }
    let scientific = format!("{value:.13e}");
    let (mantissa, exponent) = scientific.split_once('e').ok_or(VmError::InvalidBytecode)?;
    let exponent: i32 = exponent.parse().map_err(|_| VmError::InvalidBytecode)?;
    if (-4..14).contains(&exponent) {
        let precision = 13_i32
            .checked_sub(exponent)
            .and_then(|places| usize::try_from(places).ok())
            .ok_or(VmError::InvalidBytecode)?;
        let fixed = format!("{value:.precision$}");
        return Ok(if fixed.contains('.') {
            fixed.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            fixed
        });
    }
    Ok(format!(
        "{}e{exponent:+03}",
        mantissa.trim_end_matches('0').trim_end_matches('.')
    ))
}
