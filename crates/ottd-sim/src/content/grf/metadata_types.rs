use super::types::ParseLimits;

/// Client palette preference used when the file accepts either palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Palette {
    /// Original DOS palette.
    Dos,
    /// Original Windows palette.
    Windows,
}

/// Host scan bounds, separate from original loading status.
#[derive(Debug, Clone, Copy)]
pub struct ScanLimits {
    /// Maximum input length.
    pub bytes: usize,
    /// Maximum executed or skipped records.
    pub records: usize,
    /// Cumulative metadata nodes, including unknown nodes.
    pub nodes: usize,
    /// Maximum number of simultaneously open metadata branches.
    pub nesting: usize,
    /// Cumulative translated bytes, including discarded or replaced text.
    pub translated_bytes: usize,
}
impl Default for ScanLimits {
    fn default() -> Self {
        let framing = ParseLimits::default();
        Self {
            bytes: framing.bytes,
            records: framing.records,
            nodes: 65_536,
            nesting: 64,
            translated_bytes: 8 * 1024 * 1024,
        }
    }
}

/// Explicit client inputs for a fresh-registry metadata scan.
#[derive(Debug, Clone, Copy)]
pub struct ScanOptions {
    /// Preference used for unspecified or dual-palette files.
    pub default_palette: Palette,
    /// Language used by convenience selection of localized text.
    pub language: u8,
    /// Independent host resource policy.
    pub limits: ScanLimits,
}
impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            default_palette: Palette::Windows,
            language: 1,
            limits: ScanLimits::default(),
        }
    }
}

/// Raw source and native internal translation of one language definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalizedText<'a> {
    /// `NewGRF` language ID, not an old Action4 language mask.
    pub language: u8,
    /// Original bytes excluding the terminator.
    pub raw: &'a [u8],
    /// Config GRFID when this definition was encountered.
    pub definition_grfid: u32,
    /// Internal control bytes; not necessarily displayable UTF-8.
    pub translated: Vec<u8>,
}

/// Languages retain insertion order; replacement retains its original position.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextList<'a>(Vec<LocalizedText<'a>>);
impl<'a> TextList<'a> {
    /// All definitions in native encounter order.
    #[must_use]
    pub fn entries(&self) -> &[LocalizedText<'a>] {
        &self.0
    }
    /// Exact language, unspecified language, then first English definition.
    #[must_use]
    pub fn select(&self, language: u8) -> Option<&LocalizedText<'a>> {
        let mut fallback = None;
        for text in &self.0 {
            if text.language == language {
                return Some(text);
            }
            if text.language == 127 || (fallback.is_none() && text.language <= 1) {
                fallback = Some(text);
            }
        }
        fallback
    }
    pub(super) fn insert(&mut self, text: LocalizedText<'a>) {
        if let Some(previous) = self
            .0
            .iter_mut()
            .find(|entry| entry.language == text.language)
        {
            *previous = text;
        } else {
            self.0.push(text);
        }
    }
}

/// Native parameter presentation type, independent of its numeric range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterType {
    /// Unsigned number or named enumeration.
    UintEnum,
    /// Boolean presentation; existing range/default values remain unchanged.
    Bool,
}

/// Finalized user parameter metadata; physical mappings may overlap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterInfo<'a> {
    /// Physical parameter number.
    pub slot: u8,
    /// First mapped bit.
    pub first_bit: u8,
    /// Number of mapped bits;32 has original whole-word semantics.
    pub num_bits: u8,
    /// Presentation kind.
    pub kind: ParameterType,
    /// Inclusive minimum.
    pub min: u32,
    /// Inclusive maximum.
    pub max: u32,
    /// Raw default before clamping.
    pub default: u32,
    /// Localized name.
    pub name: TextList<'a>,
    /// Localized description.
    pub description: TextList<'a>,
    /// Sorted numeric labels with their localized definitions.
    pub value_names: Vec<(u32, TextList<'a>)>,
    /// Every value in the inclusive final range has a label.
    pub complete_labels: bool,
}
impl ParameterInfo<'_> {
    pub(super) fn new(slot: u8) -> Self {
        Self {
            slot,
            first_bit: 0,
            num_bits: 32,
            kind: ParameterType::UintEnum,
            min: 0,
            max: u32::MAX,
            default: 0,
            name: TextList::default(),
            description: TextList::default(),
            value_names: Vec::new(),
            complete_labels: false,
        }
    }
    /// Read mapped bits; an absent physical parameter is zero.
    #[must_use]
    pub fn read(&self, parameters: &[u32]) -> u32 {
        let value = parameters.get(usize::from(self.slot)).copied().unwrap_or(0);
        if self.num_bits == 32 {
            value
        } else {
            value.checked_shr(u32::from(self.first_bit)).unwrap_or(0) & self.mask()
        }
    }
    fn mask(&self) -> u32 {
        1_u32
            .checked_shl(u32::from(self.num_bits))
            .unwrap_or(0)
            .wrapping_sub(1)
    }
    /// Clamp and write using native uint32 truncation and vector extension.
    pub fn write(&self, parameters: &mut Vec<u32>, value: u32) {
        let slot = usize::from(self.slot);
        if parameters.len() <= slot {
            parameters.resize(slot.saturating_add(1), 0);
        }
        let value = value.max(self.min).min(self.max);
        if let Some(previous) = parameters.get_mut(slot) {
            if self.num_bits == 32 {
                *previous = value;
            } else {
                let shift = u32::from(self.first_bit);
                let mask = self.mask().checked_shl(shift).unwrap_or(0);
                *previous = (*previous & !mask) | value.checked_shl(shift).unwrap_or(0);
            }
        }
    }
    pub(super) fn finalize(&mut self) {
        self.value_names
            .retain(|(value, _)| *value >= self.min && *value <= self.max);
        self.complete_labels = u64::from(self.max)
            .saturating_sub(u64::from(self.min))
            .saturating_add(1)
            == u64::try_from(self.value_names.len()).unwrap_or(u64::MAX);
    }
}

/// FILESCAN metadata; this does not activate content or apply defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GrfStaticInfo<'a> {
    /// Localized display name.
    pub name: TextList<'a>,
    /// Localized description.
    pub description: TextList<'a>,
    /// Localized URL.
    pub url: TextList<'a>,
    /// File-declared content version.
    pub version: u32,
    /// Minimum compatible saved content version.
    pub min_loadable_version: u32,
    /// Current admitted metadata ID count.
    pub num_valid_params: u8,
    /// Native palette/blitter flags including chosen palette bit.
    pub palette_bits: u8,
    /// Any consumed DFLT node, including wrong-sized nodes.
    pub has_param_defaults: bool,
    /// Sparse metadata-index vector.
    pub parameters: Vec<Option<ParameterInfo<'a>>>,
}
impl Default for GrfStaticInfo<'_> {
    fn default() -> Self {
        Self {
            name: TextList::default(),
            description: TextList::default(),
            url: TextList::default(),
            version: 0,
            min_loadable_version: 0,
            num_valid_params: 128,
            palette_bits: 0,
            has_param_defaults: false,
            parameters: Vec::new(),
        }
    }
}
impl GrfStaticInfo<'_> {
    /// Whether this content version can replace the given saved version.
    #[must_use]
    pub const fn is_compatible(&self, old_version: u32) -> bool {
        self.min_loadable_version <= old_version && old_version <= self.version
    }
    /// Apply defaults to a fresh vector in metadata-index order.
    #[must_use]
    pub fn default_parameters(&self) -> Vec<u32> {
        let mut values = Vec::new();
        if self.has_param_defaults {
            for parameter in self.parameters.iter().flatten() {
                parameter.write(&mut values, parameter.default);
            }
        }
        values
    }
    pub(super) fn finalize(&mut self, palette: Palette) {
        let windows = match self.palette_bits & 0x0c {
            4 => false,
            8 => true,
            _ => palette == Palette::Windows,
        };
        self.palette_bits = (self.palette_bits & !1) | u8::from(windows);
        for parameter in self.parameters.iter_mut().flatten() {
            parameter.finalize();
        }
    }
}
