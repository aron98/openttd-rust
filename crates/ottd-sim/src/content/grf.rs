//! Native `NewGRF` file framing and metadata scanning, without activation.
mod action14;
mod action14_parameters;
mod container;
mod metadata_types;
mod records;
mod scan;
mod string_ids;
mod text;
mod text_choices;
mod text_codes;
mod text_reader;
mod text_translate;
mod types;
pub use container::GrfContainer;
pub use metadata_types::{
    GrfStaticInfo, LocalizedText, Palette, ParameterInfo, ParameterType, ScanLimits, ScanOptions,
    TextList,
};
pub use scan::{scan_file, scan_file_with_options};
pub use text::translate_fresh_text;
pub use types::{
    GrfIdentity, GrfParseError, Metadata, ParseLimits, Record, RecordKind, ScanError, ScanFailure,
    ScanOutcome, ScanStatus, SpriteVariant,
};
