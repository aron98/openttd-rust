//! Static safety admission from OpenTTD15.3, pin14ec60f, GPL-2.0-only.
use super::{
    GrfParseError, ScanError, ScanFailure, ScanOptions, ScanOutcome,
    load_cursor::{Cursor, Skip, count_records},
    records::Reader,
};

/// One ordered config identity used by native engine-mapping safety checks.
#[derive(Debug, Clone, Copy)]
pub struct SafetyConfig {
    /// Source-scanned identifier; duplicate identifiers retain their order.
    pub grfid: u32,
    /// Actual configuration static flag, independent of scan acceptance.
    pub is_static: bool,
}
#[derive(Clone, Copy)]
pub(super) struct SafetyContext<'a> {
    pub current_grfid: u32,
    pub configs: &'a [SafetyConfig],
}
/// Host bounds independent of native safety or disabling.
#[derive(Debug, Clone, Copy)]
pub struct SafetyLimits {
    /// Maximum source bytes.
    pub bytes: usize,
    /// Maximum visited records.
    pub records: usize,
    /// Cumulative ordered config entries inspected.
    pub registry_work: usize,
    /// Cumulative decision storage bytes.
    pub trace_bytes: usize,
}
impl Default for SafetyLimits {
    fn default() -> Self {
        Self {
            bytes: 64 * 1024 * 1024,
            records: 1_000_000,
            registry_work: 1_000_000,
            trace_bytes: 64 * 1024 * 1024,
        }
    }
}
impl SafetyLimits {
    fn decision(self, previous: usize) -> Result<(), SafetyError> {
        if previous >= self.records {
            return Err(SafetyError::Limit("records"));
        }
        let next = previous
            .checked_add(1)
            .ok_or(SafetyError::Limit("trace bytes"))?;
        if next
            .checked_mul(std::mem::size_of::<SafetyDecision>())
            .is_none_or(|size| size > self.trace_bytes)
        {
            return Err(SafetyError::Limit("trace bytes"));
        }
        Ok(())
    }
}
/// Native Unsafe flag and its first cause, separate from Disabled status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafetyOutcome {
    /// No unsafe handler was reached.
    Safe,
    /// A handler set Unsafe and stopped processing.
    Unsafe {
        /// Physical data-record line.
        line: u32,
        /// Executed action.
        action: u8,
    },
}
/// One actual safety record visit, including skipped records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafetyDecision {
    /// Physical data-record line.
    pub line: u32,
    /// Record header offset.
    pub offset: usize,
    /// Executed action; absent for skips or rejected record headers.
    pub action: Option<u8>,
    /// Bytes consumed within the executed payload.
    pub consumed: usize,
    /// Native remaining skip count, or minus one for stop.
    pub skip: i32,
}
/// Results from the separate native safety pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafetyReport {
    /// Unsafe flag/cause.
    pub outcome: SafetyOutcome,
    /// Native disabling caused by this pass, distinct from Unsafe.
    pub failure: Option<(ScanFailure, u32)>,
    /// Ordered record observations.
    pub decisions: Vec<SafetyDecision>,
}
/// Combined FILESCAN and static safety admission; never an activated catalog.
#[derive(Debug)]
pub struct StaticScan<'a> {
    /// Final admission and metadata; identity exists only if admission succeeds.
    pub scan: ScanOutcome<'a>,
    /// Absent when FILESCAN identity/system admission prevented safety.
    pub safety: Option<SafetyReport>,
}
/// Structural and host errors, never native Unsafe or Disabled.
#[derive(Debug, thiserror::Error)]
pub enum SafetyError {
    /// FILESCAN or structural failure.
    #[error(transparent)]
    Scan(#[from] ScanError),
    /// Configured cumulative host bound.
    #[error("static safety host limit: {0}")]
    Limit(&'static str),
}
impl From<GrfParseError> for SafetyError {
    fn from(error: GrfParseError) -> Self {
        Self::Scan(error.into())
    }
}

/// Run FILESCAN and then native static-safety admission in their original order.
/// # Errors
/// Returns structural scan errors or cumulative host-bound refusals.
pub fn scan_static_file<'a>(
    bytes: &'a [u8],
    options: ScanOptions,
    configs: &[SafetyConfig],
    limits: SafetyLimits,
) -> Result<StaticScan<'a>, SafetyError> {
    let mut scan = super::scan_file_with_options(bytes, options)?;
    let Some(identity) = scan.identity else {
        return Ok(StaticScan { scan, safety: None });
    };
    let report = scan_safety(
        bytes,
        SafetyContext {
            current_grfid: identity.grfid,
            configs,
        },
        limits,
    )?;
    if let Some(failure) = report.failure {
        scan.status = super::ScanStatus::Disabled;
        if scan.failure.is_none() {
            scan.failure = Some(failure);
        }
    }
    if matches!(report.outcome, SafetyOutcome::Unsafe { .. }) {
        scan.accepted = false;
        scan.identity = None;
    }
    Ok(StaticScan {
        scan,
        safety: Some(report),
    })
}

fn unsafe_action(
    action: u8,
    reader: &mut Reader<'_>,
    context: &SafetyContext<'_>,
    work: &mut usize,
    limit: usize,
) -> Result<bool, SafetyError> {
    match action {
        3 | 0x0f | 0x11 => Ok(true),
        0x0d => {
            let target = reader.byte()?;
            Ok(target >= 0x80 && target != 0x9e)
        }
        0x0e => {
            for _ in 0..reader.byte()? {
                if reader.dword()? != context.current_grfid {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        0 => {
            let feature = reader.byte()?;
            let properties = reader.byte()?;
            let count = reader.byte()?;
            reader.extended()?;
            match (feature, properties) {
                (6, 1) => Ok(reader.byte()? != 0x0d),
                (8, 1) => {
                    if reader.byte()? != 0x11 {
                        return Ok(true);
                    }
                    for _ in 0..count {
                        let source = reader.dword()?;
                        reader.dword()?;
                        for config in context.configs {
                            *work = work
                                .checked_add(1)
                                .ok_or(SafetyError::Limit("registry work"))?;
                            if *work > limit {
                                return Err(SafetyError::Limit("registry work"));
                            }
                            if config.grfid == source {
                                if !config.is_static {
                                    return Ok(true);
                                }
                                break;
                            }
                        }
                    }
                    Ok(false)
                }
                _ => Ok(true),
            }
        }
        _ => Ok(false),
    }
}

pub(super) fn scan_safety(
    bytes: &[u8],
    context: SafetyContext<'_>,
    limits: SafetyLimits,
) -> Result<SafetyReport, SafetyError> {
    if bytes.len() > limits.bytes {
        return Err(SafetyError::Limit("source bytes"));
    }
    let mut cursor = Cursor::new(bytes)?;
    let mut report = SafetyReport {
        outcome: SafetyOutcome::Safe,
        failure: None,
        decisions: Vec::new(),
    };
    let mut registry_work = 0;
    loop {
        let offset = cursor.reader.pos;
        let Some((length, kind)) = cursor.reader.record_header(cursor.version)? else {
            break;
        };
        limits.decision(report.decisions.len())?;
        cursor.line = cursor
            .line
            .checked_add(1)
            .ok_or(SafetyError::Limit("records"))?;
        let mut decision = SafetyDecision {
            line: cursor.line,
            offset,
            action: None,
            consumed: 0,
            skip: cursor.skip.value(),
        };
        if cursor.skip == Skip::None && (kind != 255 || length > 1024 * 1024) {
            report.failure = Some((ScanFailure::UnexpectedSprite, cursor.line));
            decision.skip = -1;
            report.decisions.push(decision);
            break;
        }
        match cursor.skip {
            Skip::Count(_) | Skip::Stop => {
                cursor
                    .reader
                    .skip_native_record(cursor.version, kind, length)?;
                cursor.skipped();
            }
            Skip::None => {
                let payload = cursor
                    .reader
                    .take(usize::try_from(length).map_err(|_| GrfParseError::ResourceLimit)?)?;
                let mut reader = Reader {
                    bytes: payload,
                    pos: 0,
                };
                let action = reader.byte()?;
                decision.action = Some(action);
                let handled = if matches!(action, 1 | 5 | 0x0a | 0x12) {
                    count_records(action, &mut reader)
                        .map(|count| {
                            cursor.skip = Skip::from_count(count);
                            false
                        })
                        .map_err(SafetyError::from)
                } else {
                    unsafe_action(
                        action,
                        &mut reader,
                        &context,
                        &mut registry_work,
                        limits.registry_work,
                    )
                };
                match handled {
                    Ok(true) => {
                        report.outcome = SafetyOutcome::Unsafe {
                            line: cursor.line,
                            action,
                        };
                        cursor.skip = Skip::Stop;
                    }
                    Ok(false) => (),
                    Err(SafetyError::Scan(ScanError::Structure(GrfParseError::Truncated(_)))) => {
                        report.failure = Some((ScanFailure::ReadBounds, cursor.line));
                        cursor.skip = Skip::Stop;
                    }
                    Err(error) => return Err(error),
                }
                decision.consumed = reader.pos;
            }
        }
        decision.skip = cursor.skip.value();
        report.decisions.push(decision);
        if cursor.skip == Skip::Stop {
            break;
        }
    }
    Ok(report)
}
