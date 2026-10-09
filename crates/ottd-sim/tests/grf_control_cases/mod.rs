//! Deterministic independently encoded loading-control programs.
use ottd_sim::content::grf::{GrfIdentity, LoadFlags, LoadInput, Palette};
/// Fallible fixture construction.
pub type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
/// One explicitly configured file.
#[derive(Debug)]
pub struct Source {
    pub(crate) name: String,
    pub(crate) bytes: Option<Vec<u8>>,
    pub(crate) id: u32,
    pub(crate) parameters: Vec<u32>,
    pub(crate) flags: LoadFlags,
    pub(crate) metadata_version: u32,
}
impl Source {
    pub(crate) fn input(&self) -> LoadInput<'_> {
        LoadInput {
            name: &self.name,
            bytes: self.bytes.as_deref(),
            identity: GrfIdentity {
                grfid: self.id,
                md5: [0; 16],
            },
            metadata_version: self.metadata_version,
            palette: Palette::Windows,
            parameters: &self.parameters,
            flags: self.flags,
        }
    }
}
/// One original scheduler invocation.
#[derive(Debug)]
pub struct Case {
    pub(crate) name: String,
    pub(crate) sources: Vec<Source>,
    pub(crate) networking: bool,
}
/// Build the corresponding deterministic fixture data.
/// # Errors
/// Returns an error when a generated size cannot be represented.
pub fn encode(records: &[Vec<u8>], version: u8) -> Result<Vec<u8>> {
    let mut bytes = if version == 1 {
        Vec::new()
    } else {
        b"\0\0GRF\x82\r\n\x1a\n\0\0\0\0\0".to_vec()
    };
    for record in std::iter::once(vec![0; 4]).chain(records.iter().cloned()) {
        if version == 1 {
            bytes.extend(u16::try_from(record.len())?.to_le_bytes());
        } else {
            bytes.extend(u32::try_from(record.len())?.to_le_bytes());
        }
        bytes.push(255);
        bytes.extend(record);
    }
    bytes.extend(vec![0; if version == 1 { 2 } else { 4 }]);
    if version == 2 {
        let offset = u32::try_from(bytes.len().checked_sub(14).ok_or("offset")?)?;
        bytes
            .get_mut(10..14)
            .ok_or("offset field")?
            .copy_from_slice(&offset.to_le_bytes());
        bytes.extend([0; 4]);
    }
    Ok(bytes)
}
/// Encode the original action fields.
pub fn info(id: u32) -> Vec<u8> {
    let mut bytes = vec![8, 8];
    bytes.extend(id.to_le_bytes());
    bytes.extend(b"control\0description\0");
    bytes
}
/// Build the corresponding deterministic fixture data.
/// # Errors
/// Returns an error when a generated size cannot be represented.
pub fn source(id: u32, records: &[Vec<u8>], parameters: &[u32], version: u8) -> Result<Source> {
    let all = std::iter::once(info(id))
        .chain(records.iter().cloned())
        .collect::<Vec<_>>();
    Ok(Source {
        name: format!("{id:08x}.grf"),
        bytes: Some(encode(&all, version)?),
        id,
        parameters: parameters.to_vec(),
        flags: LoadFlags::default(),
        metadata_version: 17,
    })
}
/// Build the corresponding deterministic fixture data.
/// # Errors
/// Returns an error when a generated size cannot be represented.
pub fn single(name: &str, records: &[Vec<u8>], parameters: &[u32], version: u8) -> Result<Case> {
    Ok(Case {
        name: name.into(),
        sources: vec![source(0x4141_4141, records, parameters, version)?],
        networking: false,
    })
}
/// Encode the original action fields.
pub fn set(target: u8, operation: u8, first: u8, second: u8, data: u32) -> Vec<u8> {
    let mut bytes = vec![0x0d, target, operation, first, second];
    bytes.extend(data.to_le_bytes());
    bytes
}
/// Encode the original action fields.
pub fn condition(
    action: u8,
    parameter: u8,
    width: u8,
    kind: u8,
    value: u32,
    mask: u32,
    count: u8,
) -> Vec<u8> {
    let mut bytes = vec![action, parameter, width, kind];
    let actual = if kind < 2 { 1 } else { width };
    match actual {
        1 => bytes.extend(value.to_le_bytes().into_iter().take(1)),
        2 => bytes.extend(value.to_le_bytes().into_iter().take(2)),
        4 | 8 => bytes.extend(value.to_le_bytes()),
        _ => (),
    }
    if actual == 8 {
        bytes.extend(mask.to_le_bytes());
    }
    bytes.push(count);
    bytes
}
mod arithmetic;
mod boundaries;
mod branches;
mod mutations;
mod reexecution;
/// Build the corresponding deterministic fixture data.
/// # Errors
/// Returns an error when a generated size cannot be represented.
pub fn all() -> Result<Vec<Case>> {
    let mut cases = arithmetic::cases()?;
    cases.extend(branches::cases()?);
    cases.extend(mutations::cases()?);
    cases.extend(boundaries::cases()?);
    cases.extend(reexecution::cases()?);
    Ok(cases)
}
