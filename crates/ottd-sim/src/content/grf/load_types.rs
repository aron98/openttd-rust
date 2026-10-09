use super::{GrfIdentity, GrfParseError, Palette};

/// Original loading stage; actual loading runs `LabelScan` through `Activation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LoadStage {
    /// Metadata discovery.
    FileScan,
    /// Static-file safety classification.
    SafetyScan,
    /// Physical label discovery.
    LabelScan,
    /// Initial parameter execution.
    Init,
    /// Shared resource reservation.
    Reserve,
    /// Final action execution before catalog finalization.
    Activation,
}

/// Original per-config load status, independent of host support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadStatus {
    /// No Action8 has initialized this config.
    Unknown,
    /// A native failure disabled this config.
    Disabled,
    /// Configured source is absent.
    NotFound,
    /// Initialization or a stage reset has completed.
    Initialised,
    /// Action8 executed during reservation or activation.
    Activated,
}

/// Config flags that affect loading order and cross-file queries.
#[derive(Debug, Clone, Copy, Default)]
pub struct LoadFlags {
    /// Locally installed static file.
    pub is_static: bool,
    /// Stop after initialization.
    pub init_only: bool,
    /// Exclude an original system file from the network file-count limit.
    pub system: bool,
}

/// Immutable configured input; parameters are already explicitly chosen.
#[derive(Debug, Clone, Copy)]
pub struct LoadInput<'a> {
    /// Native filename identity used for dynamic-file reuse.
    pub name: &'a str,
    /// Missing source is distinct from an empty malformed file.
    pub bytes: Option<&'a [u8]>,
    /// Scanned or saved identity, retained for missing sources.
    pub identity: GrfIdentity,
    /// Action14 content version for external version queries.
    pub metadata_version: u32,
    /// Already selected client palette.
    pub palette: Palette,
    /// Configured raw parameter vector, without hidden default application.
    pub parameters: &'a [u32],
    /// Original control flags.
    pub flags: LoadFlags,
}

/// Cumulative host work bounds; these never fabricate native Disabled status.
#[derive(Debug, Clone, Copy)]
pub struct ControlOptions {
    /// Enable original static/non-static network influence rules.
    pub networking: bool,
    /// Maximum cumulative physical record visits, including jumps.
    pub max_steps: usize,
    /// Maximum cumulative override bytes copied or written.
    pub max_override_bytes: usize,
    /// Maximum trace events.
    pub max_trace_events: usize,
    /// Maximum cumulative trace payload and snapshot bytes.
    pub max_trace_bytes: usize,
    /// Maximum configured file count, including static/system files.
    pub max_files: usize,
    /// Maximum total source bytes.
    pub max_source_bytes: usize,
    /// Maximum total label definitions.
    pub max_labels: usize,
}
impl Default for ControlOptions {
    fn default() -> Self {
        Self {
            networking: false,
            max_steps: 1_000_000,
            max_override_bytes: 64 * 1024 * 1024,
            max_trace_events: 1_000_000,
            max_trace_bytes: 64 * 1024 * 1024,
            max_files: 1024,
            max_source_bytes: 256 * 1024 * 1024,
            max_labels: 65_536,
        }
    }
}

/// Physical source location and original NFO line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadLocation {
    /// Executing native phase.
    pub stage: LoadStage,
    /// Ordered configuration index.
    pub file: usize,
    /// One-based physical record line; zero at phase entry.
    pub line: u32,
    /// Physical record start offset.
    pub offset: usize,
}

/// Original failure categories used by the control-only profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadFailure {
    /// An executed pseudo action lacks required bytes.
    ReadBounds,
    /// Unrequested real sprite or oversized executed pseudo sprite.
    UnexpectedSprite,
    /// Second Action8 during initialization.
    MultipleAction8,
    /// Networked non-static file queried a static file.
    StaticInfluence,
    /// More than255 non-static/non-system files.
    TooManyFiles,
}

/// Original diagnostic retained separately from a host refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadDiagnostic {
    /// Native error class.
    pub failure: LoadFailure,
    /// Current executing NFO line, including external-file disables.
    pub line: u32,
}

/// Native label destination retained until file cleanup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoadLabel {
    /// One-byte label identifier.
    pub id: u8,
    /// Physical NFO line containing the label.
    pub line: u32,
    /// Physical cursor position immediately after the label record.
    pub offset: usize,
}

/// A config snapshot with optional dynamic-file state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileControlState {
    /// Original configured GRFID.
    pub config_grfid: u32,
    /// Dynamic identity exists only after native file initialization.
    pub file_grfid: Option<u32>,
    /// Action8 language version, when a dynamic file exists.
    pub version: Option<u8>,
    /// Config status.
    pub status: LoadStatus,
    /// Native reservation flag.
    pub reserved: bool,
    /// Dynamic parameter vector; missing files do not invent one.
    pub parameters: Option<Vec<u32>>,
    /// Dynamic labels; absent only when no dynamic file exists.
    pub labels: Option<Vec<LoadLabel>>,
    /// Original errors in encounter order.
    pub errors: Vec<LoadDiagnostic>,
}

/// An Action6 override, keyed by configured identity and physical line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverrideState {
    /// Config identity, not dynamic Action8 identity.
    pub config_grfid: u32,
    /// Target NFO line.
    pub line: u32,
    /// Current mutable pseudo payload.
    pub bytes: Vec<u8>,
}

/// Bounded decisions and snapshots from actual control execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadEvent {
    /// Begin one original phase.
    StageStart(LoadStage),
    /// State before phase cleanup or status reset.
    StageEnd {
        /// Original loading phase.
        stage: LoadStage,
        /// Ordered config and dynamic snapshots.
        files: Vec<FileControlState>,
        /// Ordered mutable pseudo-sprite payloads.
        overrides: Vec<OverrideState>,
    },
    /// Physical record visit, including records not dispatched.
    Record {
        /// Executing source location.
        location: LoadLocation,
        /// Executed or requested action byte.
        action: Option<u8>,
        /// Whether native dispatch was entered.
        executed: bool,
    },
    /// Post-record native cursor and mutable registry state.
    Decision {
        /// Executing source location before any jump.
        location: LoadLocation,
        /// Native skip counter; minus one terminates this file.
        skip: i32,
        /// Current NFO line after any jump.
        next_line: u32,
        /// Physical cursor after record handling.
        next_offset: usize,
        /// Ordered config and dynamic state.
        files: Vec<FileControlState>,
        /// Current mutable sprite overrides.
        overrides: Vec<OverrideState>,
    },
    /// Label points after its physical record.
    Label {
        /// Executing source location.
        location: LoadLocation,
        /// Original one-byte label ID.
        label: u8,
        /// Physical cursor position after the label record.
        post_record_offset: usize,
    },
    /// Dynamic parameter state after an executed write.
    Parameters {
        /// Executing source location.
        location: LoadLocation,
        /// Complete dynamic parameter vector.
        values: Vec<u32>,
    },
    /// Mutable next-sprite state after Action6.
    Override {
        /// Executing source location.
        location: LoadLocation,
        /// Destination NFO line.
        target_line: u32,
        /// Current override payload bytes.
        bytes: Vec<u8>,
    },
    /// Original label jump destination.
    Jump {
        /// Executing source location.
        location: LoadLocation,
        /// Destination NFO line.
        target_line: u32,
        /// Physical jump destination.
        target_offset: usize,
    },
    /// Config status transition, possibly targeting another file.
    Status {
        /// Executing source location.
        location: LoadLocation,
        /// Affected ordered configuration index.
        target_file: usize,
        /// Original resulting config status.
        status: LoadStatus,
    },
}

/// Control execution only; this cannot serve as an activated gameplay catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlLoadReport {
    /// Final config/dynamic states after all four passes.
    pub files: Vec<FileControlState>,
    /// Bounded exact execution observations.
    pub events: Vec<LoadEvent>,
}

/// Host refusals are distinct from original per-config failures.
#[derive(Debug, thiserror::Error)]
pub enum ControlLoadError {
    /// Unsafe physical framing or source structure.
    #[error("GRF source {file}: {source}")]
    Structural {
        /// Configured input index.
        file: usize,
        /// Original structural parser error.
        source: GrfParseError,
    },
    /// Executed behavior outside the admitted control profile.
    #[error("unsupported control action {action:#x} at {location:?}: {detail}")]
    Unsupported {
        /// Executing source location.
        location: LoadLocation,
        /// Executed or requested action byte.
        action: u8,
        /// Specific unsupported or invalid domain.
        detail: &'static str,
    },
    /// Cumulative host work limit.
    #[error("GRF control {resource} limit at {location:?}")]
    ResourceLimit {
        /// Executing source location.
        location: LoadLocation,
        /// Cumulative host resource category.
        resource: &'static str,
    },
    /// Input would invoke undefined arithmetic or an invalid native invariant.
    #[error("invalid native domain at {location:?}: {detail}")]
    InvalidNativeDomain {
        /// Executing source location.
        location: LoadLocation,
        /// Specific unsupported or invalid domain.
        detail: &'static str,
    },
}
