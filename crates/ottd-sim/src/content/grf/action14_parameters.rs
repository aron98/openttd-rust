use super::{
    metadata_types::{GrfStaticInfo, ParameterInfo, ParameterType},
    records::Reader,
    types::GrfParseError,
};

pub(super) fn info_binary(
    info: &mut GrfStaticInfo<'_>,
    id: [u8; 4],
    bytes: &[u8],
) -> Result<(), GrfParseError> {
    let mut reader = Reader { bytes, pos: 0 };
    match (&id, bytes.len()) {
        (b"NPAR", 1) => info.num_valid_params = reader.byte()?.min(128),
        (b"PALS", 1) => {
            let palette = match reader.byte()? {
                b'D' => Some(4),
                b'W' => Some(8),
                b'A' | b'*' => Some(12),
                _ => None,
            };
            if let Some(palette) = palette {
                info.palette_bits = (info.palette_bits & !0x0c) | palette;
            }
        }
        (b"BLTR", 1) => match reader.byte()? {
            b'3' => info.palette_bits |= 16,
            b'8' => info.palette_bits &= !16,
            _ => (),
        },
        (b"VRSN", 4) => {
            info.version = reader.dword()?;
            info.min_loadable_version = info.version;
        }
        (b"MINV", 4) => info.min_loadable_version = reader.dword()?.min(info.version),
        _ => (),
    }
    Ok(())
}

pub(super) fn parameter_binary(
    parameter: &mut ParameterInfo<'_>,
    id: [u8; 4],
    bytes: &[u8],
) -> Result<(), GrfParseError> {
    let mut reader = Reader { bytes, pos: 0 };
    match (&id, bytes.len()) {
        (b"TYPE", 1) => match reader.byte()? {
            0 => parameter.kind = ParameterType::UintEnum,
            1 => parameter.kind = ParameterType::Bool,
            _ => (),
        },
        (b"LIMI", 8) if parameter.kind == ParameterType::UintEnum => {
            let min = reader.dword()?;
            let max = reader.dword()?;
            if min <= max {
                parameter.min = min;
                parameter.max = max;
            }
        }
        (b"MASK", 1..=3) => {
            let slot = reader.byte()?;
            if slot < 128 {
                parameter.slot = slot;
                if reader.remaining() > 0 {
                    parameter.first_bit = reader.byte()?.min(31);
                }
                if reader.remaining() > 0 {
                    parameter.num_bits = reader
                        .byte()?
                        .min(32_u8.saturating_sub(parameter.first_bit));
                }
            }
        }
        (b"DFLT", 4) => parameter.default = reader.dword()?,
        _ => (),
    }
    Ok(())
}
