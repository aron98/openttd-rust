//! Native `NewGRF` file framing and metadata scanning, without activation.
mod container;
mod records;
mod scan;
mod types;
pub use container::GrfContainer;
pub use scan::scan_file;
pub use types::{
    GrfIdentity, GrfParseError, Metadata, ParseLimits, Record, RecordKind, ScanError, ScanFailure,
    ScanOutcome, ScanStatus, SpriteVariant,
};
