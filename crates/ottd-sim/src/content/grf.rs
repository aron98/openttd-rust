//! Native `NewGRF` file framing and metadata scanning, without activation.
mod action14;
mod action14_parameters;
mod container;
#[cfg(test)]
mod language_native;
mod language_pack;
#[cfg(test)]
mod language_pack_tests;
mod load;
mod load_budget;
mod load_conditions;
mod load_context;
#[cfg(test)]
pub mod load_context_tests;
mod load_currency;
#[cfg(test)]
mod load_currency_api_native;
mod load_currency_data;
#[cfg(test)]
mod load_currency_guards;
#[cfg(test)]
mod load_currency_load_native;
#[cfg(test)]
mod load_currency_native;
#[cfg(test)]
mod load_currency_session_tests;
#[cfg(test)]
mod load_currency_tests;
mod load_cursor;
mod load_dispatch;
mod load_execute;
mod load_globals;
mod load_language;
mod load_language_state;
mod load_parameters;
mod load_patch;
mod load_registry;
mod load_string_actions;
#[cfg(test)]
mod load_string_actions_tests;
mod load_string_mapping;
mod load_strings;
#[cfg(test)]
mod load_strings_guards;
#[cfg(test)]
mod load_strings_load_native;
#[cfg(test)]
mod load_strings_native;
#[cfg(test)]
mod load_strings_tests;
mod load_substitution;
mod load_types;
mod metadata_types;
mod records;
mod safety;
#[cfg(test)]
mod safety_cases;
#[cfg(test)]
mod safety_guards;
#[cfg(test)]
mod safety_native;
mod scan;
pub use safety::{
    SafetyConfig, SafetyDecision, SafetyError, SafetyLimits, SafetyOutcome, SafetyReport,
    StaticScan, scan_static_file,
};
#[cfg(test)]
mod safety_tests;
mod string_ids;
mod text;
mod text_choices;
mod text_codes;
mod text_mapped;
mod text_reader;
mod text_translate;
mod types;
pub use container::GrfContainer;
pub use load::{run_control_load, run_control_load_with_prefix};
pub use load_types::{
    ControlLoadError, ControlLoadReport, ControlOptions, FileControlState, LoadDiagnostic,
    LoadEvent, LoadFailure, LoadFlags, LoadInput, LoadLabel, LoadLocation, LoadStage, LoadStatus,
    OverrideState,
};
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
