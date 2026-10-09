use std::ops::Range;

/// Host admission bounds, independent of native loading status.
#[derive(Debug, Clone, Copy)]
pub struct ParseLimits {
    /// Largest admitted source file.
    pub bytes: usize,
    /// Maximum combined number of data records and sprite variants.
    pub records: usize,
}
impl Default for ParseLimits {
    fn default() -> Self {
        Self {
            bytes: 256 * 1024 * 1024,
            records: 1_000_000,
        }
    }
}

/// Structural errors never imply native GRF activation or disabling.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GrfParseError {
    /// A field extends beyond available bytes.
    #[error("truncated GRF at byte {0}")]
    Truncated(usize),
    /// The container signature is not recognized.
    #[error("invalid GRF signature")]
    Signature,
    /// Native supports only uncompressed v2 data sections.
    #[error("unsupported GRF container compression {0}")]
    Compression(u8),
    /// Initial pseudo sprite must be four bytes.
    #[error("invalid GRF initial count record")]
    InitialRecord,
    /// Sprite-section offset cannot delimit the data section.
    #[error("invalid GRF sprite-section offset")]
    SectionOffset,
    /// Inline sprite cannot match its declared decoded length.
    #[error("invalid inline sprite at byte {0}")]
    InlineSprite(usize),
    /// A caller-configured resource bound was exceeded.
    #[error("GRF resource limit exceeded")]
    ResourceLimit,
}

/// A data-section record preserves original bytes and source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record<'a> {
    /// Complete physical record span, including length/type.
    pub span: Range<usize>,
    /// Declared length, which differs from compressed physical length.
    pub declared_length: u32,
    /// Borrowed record contents.
    pub kind: RecordKind<'a>,
}

/// Native record categories; pseudo bytes are not assumed to be executable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordKind<'a> {
    /// Pseudo action, import or sound-data bytes.
    Pseudo(&'a [u8]),
    /// V2 sprite reference payload, retained verbatim.
    SpriteReference(&'a [u8]),
    /// V1-style real sprite, including metadata and compressed/raw payload.
    InlineSprite {
        /// Native type flags.
        flags: u8,
        /// Seven metadata bytes followed by physical pixel data.
        bytes: &'a [u8],
    },
}

/// One v2 sprite-section entry, including duplicate IDs and zoom variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpriteVariant<'a> {
    /// Native sprite identifier.
    pub id: u32,
    /// Complete physical entry span.
    pub span: Range<usize>,
    /// Original payload including component/zoom bytes.
    pub bytes: &'a [u8],
}

/// Native legacy content identity, not an authenticity guarantee.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrfIdentity {
    /// Native little-endian GRFID.
    pub grfid: u32,
    /// MD5 over the native-defined data extent.
    pub md5: [u8; 16],
}

/// Action 8 metadata retains unconverted GRF string bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata<'a> {
    /// GRF language version, separate from container version.
    pub version: u8,
    /// GRFID even when zero or reserved.
    pub grfid: u32,
    /// Raw name without optional terminating NUL.
    pub name: &'a [u8],
    /// Optional raw description; an explicitly empty string remains present.
    pub info: Option<&'a [u8]>,
}

/// Status values possible during FILESCAN, not initialization/activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanStatus {
    /// No activation stage has executed.
    Unknown,
    /// Native scan rejected an executed malformed or unexpected sprite.
    Disabled,
}

/// Original FILESCAN error classes, separate from structural parser failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanFailure {
    /// Executed pseudo sprite does not contain required fields.
    ReadBounds,
    /// An unrequested real sprite or oversized pseudo sprite was encountered.
    UnexpectedSprite,
}

/// FILESCAN result independent of complete container validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanOutcome<'a> {
    /// Native error class and one-based data record number, if disabled.
    pub failure: Option<(ScanFailure, u32)>,
    /// Native FILESCAN status.
    pub status: ScanStatus,
    /// Native `FillGRFDetails` eligibility for a non-static file.
    pub accepted: bool,
    /// Available only for a nonzero GRFID with a native calculated checksum.
    pub identity: Option<GrfIdentity>,
    /// First executed Action 8, if any.
    pub metadata: Option<Metadata<'a>>,
    /// GRF version lies outside native supported range2..8.
    pub invalid_version: bool,
    /// Reserved low GRFID byteFF.
    pub system: bool,
}

/// Unsupported Rust scanner behavior is distinct from native disabling.
#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    /// Container could not be safely read.
    #[error(transparent)]
    Structure(#[from] GrfParseError),
    /// Metadata action has not yet been implemented.
    #[error("unsupported metadata scan action {0:#x}")]
    UnsupportedAction(u8),
}
