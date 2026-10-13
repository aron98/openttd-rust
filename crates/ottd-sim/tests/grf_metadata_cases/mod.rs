/// Framing, action ordering and palette boundary cases.
pub mod framing;
/// Parameter IDs, ranges, mappings and labels.
pub mod parameters;
/// Exact Rust projection for independent native comparisons.
pub mod project;
/// Localized metadata and direct text-control sweeps.
pub mod text;

/// Fallible test construction or comparison.
pub type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
/// Named ordered action streams.
pub type Cases = Vec<(String, Vec<Vec<u8>>)>;
/// Encode one binary metadata node.
/// # Errors
/// Returns malformed input, framing overflow or projection errors.
pub fn binary(id: [u8; 4], bytes: &[u8]) -> Result<Vec<u8>> {
    let mut node = vec![b'B'];
    node.extend(id);
    node.extend(u16::try_from(bytes.len())?.to_le_bytes());
    node.extend(bytes);
    Ok(node)
}
/// Encode an explicitly terminated branch.
pub fn branch(id: [u8; 4], nodes: &[Vec<u8>]) -> Vec<u8> {
    let mut node = vec![b'C'];
    node.extend(id);
    for child in nodes {
        node.extend(child);
    }
    node.push(0);
    node
}
/// Encode one language definition.
pub fn text(id: [u8; 4], language: u8, bytes: &[u8]) -> Vec<u8> {
    let mut node = vec![b'T'];
    node.extend(id);
    node.push(language);
    node.extend(bytes);
    node.push(0);
    node
}
/// Wrap INFO children in Action14.
pub fn info(nodes: &[Vec<u8>]) -> Vec<u8> {
    let mut action = vec![0x14];
    action.extend(branch(*b"INFO", nodes));
    action.push(0);
    action
}
/// Build the independent Action8 identity record.
pub fn action8() -> Vec<u8> {
    b"\x08\x08TESTname\0".to_vec()
}
/// Wrap a sparse metadata parameter branch.
pub fn parameter(id: u32, nodes: &[Vec<u8>]) -> Vec<u8> {
    branch(*b"PARA", &[branch(id.to_le_bytes(), nodes)])
}
/// Frame records in the selected native container.
/// # Errors
/// Returns malformed input, framing overflow or projection errors.
pub fn file(actions: &[Vec<u8>], version: u8) -> Result<Vec<u8>> {
    let mut out = if version == 1 {
        Vec::new()
    } else {
        b"\0\0GRF\x82\r\n\x1a\n\0\0\0\0\0".to_vec()
    };
    for action in std::iter::once([0; 4].as_slice()).chain(actions.iter().map(Vec::as_slice)) {
        if version == 1 {
            out.extend(u16::try_from(action.len())?.to_le_bytes());
        } else {
            out.extend(u32::try_from(action.len())?.to_le_bytes());
        }
        out.push(255);
        out.extend(action);
    }
    out.extend(vec![0; if version == 1 { 2 } else { 4 }]);
    if version == 2 {
        let offset = u32::try_from(out.len().checked_sub(14).ok_or("offset")?)?;
        out.get_mut(10..14)
            .ok_or("header")?
            .copy_from_slice(&offset.to_le_bytes());
        out.extend([0; 4]);
    }
    Ok(out)
}
/// Generate the complete boundary scenarios.
/// # Errors
/// Returns malformed input, framing overflow or projection errors.
pub fn cases() -> Result<Cases> {
    let mut cases = framing::cases()?;
    cases.extend(parameters::cases()?);
    cases.extend(text::metadata());
    Ok(cases)
}
