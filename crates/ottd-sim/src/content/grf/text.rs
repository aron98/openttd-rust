use super::{
    metadata_types::{LocalizedText, ScanLimits},
    types::ScanError,
};

pub(super) struct Budget {
    limits: ScanLimits,
    nodes: usize,
    emitted: usize,
}
impl Budget {
    pub(super) const fn new(limits: ScanLimits) -> Self {
        Self {
            limits,
            nodes: 0,
            emitted: 0,
        }
    }
    pub(super) fn node(&mut self, offset: usize) -> Result<(), ScanError> {
        self.nodes = self.nodes.checked_add(1).ok_or(ScanError::ResourceLimit {
            resource: "nodes",
            offset,
        })?;
        if self.nodes > self.limits.nodes {
            return Err(ScanError::ResourceLimit {
                resource: "nodes",
                offset,
            });
        }
        Ok(())
    }
    pub(super) const fn depth(&self, depth: usize, offset: usize) -> Result<(), ScanError> {
        if depth > self.limits.nesting {
            return Err(ScanError::ResourceLimit {
                resource: "nesting",
                offset,
            });
        }
        Ok(())
    }
    pub(super) fn emit(&mut self, count: usize, offset: usize) -> Result<(), ScanError> {
        self.emitted = self
            .emitted
            .checked_add(count)
            .ok_or(ScanError::ResourceLimit {
                resource: "translated bytes",
                offset,
            })?;
        if self.emitted > self.limits.translated_bytes {
            return Err(ScanError::ResourceLimit {
                resource: "translated bytes",
                offset,
            });
        }
        Ok(())
    }
}

pub(super) fn localized<'a>(
    raw: &'a [u8],
    language: u8,
    definition_grfid: u32,
    newlines: bool,
    budget: &mut Budget,
    offset: usize,
) -> Result<LocalizedText<'a>, ScanError> {
    let translated = super::text_translate::translate(raw, newlines, budget, offset)?;
    Ok(LocalizedText {
        language,
        raw,
        definition_grfid,
        translated,
    })
}

/// Translate original text controls with no custom strings or language maps.
/// # Errors
/// Returns a host limit error when cumulative emitted bytes exceed the bound.
pub fn translate_fresh_text(
    raw: &[u8],
    allow_newlines: bool,
    max_emitted_bytes: usize,
) -> Result<Vec<u8>, ScanError> {
    let mut budget = Budget::new(ScanLimits {
        translated_bytes: max_emitted_bytes,
        ..ScanLimits::default()
    });
    super::text_translate::translate(raw, allow_newlines, &mut budget, 0)
}
