//! Control semantics ported from OpenTTD 15.3 `newgrf.cpp` and actions 06/07/08/09/0D/10.
//! Native source pin: 14ec60f248547d4d062a1160f0fc26d742319888 (GPL-2.0-only).

use super::{
    GrfContainer, GrfParseError,
    load_budget::Budget,
    load_registry::Registry,
    load_types::{
        ControlLoadError, ControlLoadReport, ControlOptions, LoadEvent, LoadFailure, LoadInput,
        LoadLocation, LoadStage, LoadStatus, OverrideState,
    },
};
use std::{collections::BTreeMap, sync::Arc};

pub(super) enum ActionError {
    Bounds,
    Host(ControlLoadError),
}
impl From<GrfParseError> for ActionError {
    fn from(_: GrfParseError) -> Self {
        Self::Bounds
    }
}
impl From<ControlLoadError> for ActionError {
    fn from(error: ControlLoadError) -> Self {
        Self::Host(error)
    }
}
pub(super) type ActionResult<T = ()> = Result<T, ActionError>;

pub(super) struct Session<'i, 'a> {
    pub inputs: &'i [LoadInput<'a>],
    pub preceding_ids: &'i [u32],
    pub options: ControlOptions,
    pub registry: Registry<'a>,
    pub budget: Budget,
    pub overrides: BTreeMap<(u32, u32), Arc<[u8]>>,
    pub events: Vec<LoadEvent>,
}
impl Session<'_, '_> {
    pub(super) fn check_preceding(
        &self,
        id: u32,
        mask: u32,
        location: LoadLocation,
        action: u8,
    ) -> Result<(), ControlLoadError> {
        if self
            .preceding_ids
            .iter()
            .any(|preceding| preceding & mask == id & mask)
        {
            return Err(Self::unsupported(
                location,
                action,
                "lookup selects unexecuted preceding config or dynamic file",
            ));
        }
        Ok(())
    }
    pub(super) fn emit(
        &mut self,
        event: LoadEvent,
        bytes: usize,
        location: LoadLocation,
    ) -> Result<(), ControlLoadError> {
        self.budget.trace(bytes, location)?;
        self.events.push(event);
        Ok(())
    }
    pub(super) const fn unsupported(
        location: LoadLocation,
        action: u8,
        detail: &'static str,
    ) -> ControlLoadError {
        ControlLoadError::Unsupported {
            location,
            action,
            detail,
        }
    }
    pub(super) fn disable(
        &mut self,
        target: usize,
        location: LoadLocation,
        failure: Option<LoadFailure>,
    ) -> Result<(), ControlLoadError> {
        self.registry
            .disable(target, location.file, location.line, failure);
        self.emit(
            LoadEvent::Status {
                location,
                target_file: target,
                status: LoadStatus::Disabled,
            },
            0,
            location,
        )
    }
    pub(super) fn parameter(
        &self,
        number: u8,
        location: LoadLocation,
        action: u8,
    ) -> Result<u32, ControlLoadError> {
        if number < 0x80 {
            return Ok(self
                .registry
                .file(location.file)
                .and_then(|f| f.parameters.get(usize::from(number)))
                .copied()
                .unwrap_or(0));
        }
        if number == 0x84 {
            return Ok(match location.stage {
                LoadStage::FileScan
                | LoadStage::SafetyScan
                | LoadStage::LabelScan
                | LoadStage::Init => 0,
                LoadStage::Reserve => 0x101,
                LoadStage::Activation => 0x201,
            });
        }
        Err(Self::unsupported(
            location,
            action,
            "global variable outside control-1",
        ))
    }
    fn activation_identity(
        &self,
        index: usize,
        grfid: u32,
        location: LoadLocation,
    ) -> Result<(), ControlLoadError> {
        let expected = self
            .registry
            .files
            .iter()
            .position(|file| file.grfid == grfid);
        if expected != self.registry.file_index(index) {
            return Err(ControlLoadError::InvalidNativeDomain {
                location,
                detail: "activation first-GRFID file invariant",
            });
        }
        Ok(())
    }
    fn phase(&mut self, stage: LoadStage) -> Result<(), ControlLoadError> {
        let base = LoadLocation {
            stage,
            file: 0,
            line: 0,
            offset: 0,
        };
        for config in &mut self.registry.configs {
            if config.status == LoadStatus::Activated {
                config.status = LoadStatus::Initialised;
            }
        }
        self.emit(LoadEvent::StageStart(stage), 0, base)?;
        let mut non_static = 0_usize;
        for index in 0..self.inputs.len() {
            let location = LoadLocation {
                file: index,
                ..base
            };
            let Some(input) = self.inputs.get(index).copied() else {
                continue;
            };
            if matches!(
                self.registry.status(index),
                LoadStatus::Disabled | LoadStatus::NotFound
            ) || (stage > LoadStage::Init && input.flags.init_only)
            {
                continue;
            }
            if input.bytes.is_none() {
                if let Some(config) = self.registry.configs.get_mut(index) {
                    config.status = LoadStatus::NotFound;
                }
                self.emit(
                    LoadEvent::Status {
                        location,
                        target_file: index,
                        status: LoadStatus::NotFound,
                    },
                    0,
                    location,
                )?;
                continue;
            }
            if stage == LoadStage::LabelScan {
                self.registry.initialize(input);
            }
            if !input.flags.is_static && !input.flags.system {
                if non_static == 255 {
                    self.disable(index, location, Some(LoadFailure::TooManyFiles))?;
                    continue;
                }
                non_static = non_static.saturating_add(1);
            }
            let enter = match stage {
                LoadStage::Reserve => self.registry.status(index) == LoadStatus::Initialised,
                LoadStage::Activation => {
                    self.registry.configs.get(index).is_some_and(|c| c.reserved)
                }
                LoadStage::FileScan
                | LoadStage::SafetyScan
                | LoadStage::LabelScan
                | LoadStage::Init => true,
            };
            if enter {
                self.run_file(location)?;
            }
            if stage == LoadStage::Reserve {
                if let Some(config) = self.registry.configs.get_mut(index) {
                    config.reserved = true;
                }
            }
            if stage == LoadStage::Activation || (stage == LoadStage::Init && input.flags.init_only)
            {
                if stage == LoadStage::Activation {
                    self.activation_identity(index, input.identity.grfid, location)?;
                }
                if let Some(config) = self.registry.configs.get_mut(index) {
                    if stage == LoadStage::Activation {
                        config.reserved = false;
                    }
                }
                if let Some(file) = self.registry.file_mut(index) {
                    file.labels.clear();
                }
            }
        }
        self.snapshot(stage, base)?;
        Ok(())
    }
    pub(super) fn snapshot_bytes(&self) -> usize {
        self.registry
            .snapshot_bytes()
            .saturating_add(self.overrides.values().fold(0_usize, |total, b| {
                total
                    .saturating_add(b.len())
                    .saturating_add(std::mem::size_of::<OverrideState>())
            }))
    }
    pub(super) fn override_snapshot(&self) -> Vec<OverrideState> {
        self.overrides
            .iter()
            .map(|(&(config_grfid, line), bytes)| OverrideState {
                config_grfid,
                line,
                bytes: bytes.to_vec(),
            })
            .collect()
    }
    fn snapshot(&mut self, stage: LoadStage, base: LoadLocation) -> Result<(), ControlLoadError> {
        self.budget.trace(self.snapshot_bytes(), base)?;
        self.events.push(LoadEvent::StageEnd {
            stage,
            files: self.registry.snapshots(),
            overrides: self.override_snapshot(),
        });
        Ok(())
    }
}

/// Execute a standalone configured batch, without baseline files or gameplay activation.
/// # Errors
/// Returns structural, unsupported, invalid-native-domain or cumulative host-limit errors.
pub fn run_control_load(
    inputs: &[LoadInput<'_>],
    options: ControlOptions,
) -> Result<ControlLoadReport, ControlLoadError> {
    run_control_load_with_prefix(inputs, &[], options)
}

/// Execute a configured batch after an unexecuted identity-only registry prefix.
///
/// Prefix identities must cover both preceding config and dynamic-file identities.
/// The caller must derive these from actual sources and establish they remain unchanged
/// throughout the four passes. Any lookup matching the prefix is unsupported; no
/// preceding status, parameters, or specification state is fabricated or imported.
/// Preceding filenames must be distinct from this batch's filename identities.
/// Prefix files must be static/system files excluded from the native non-static limit.
/// This primitive does not replace loading the baseline files for gameplay.
/// # Errors
/// Returns structural, unsupported, invalid-native-domain or cumulative host-limit errors.
pub fn run_control_load_with_prefix(
    inputs: &[LoadInput<'_>],
    preceding_ids: &[u32],
    options: ControlOptions,
) -> Result<ControlLoadReport, ControlLoadError> {
    let location = LoadLocation {
        stage: LoadStage::LabelScan,
        file: 0,
        line: 0,
        offset: 0,
    };
    if inputs.len().saturating_add(preceding_ids.len()) > options.max_files {
        return Err(ControlLoadError::ResourceLimit {
            location,
            resource: "configured files",
        });
    }
    let mut size = preceding_ids.len().saturating_mul(4);
    let mut sources = BTreeMap::new();
    for (file, input) in inputs.iter().enumerate() {
        if let Some(previous) = sources.insert(input.name, input.bytes) {
            if previous != input.bytes {
                return Err(ControlLoadError::InvalidNativeDomain {
                    location: LoadLocation { file, ..location },
                    detail: "one filename supplies inconsistent immutable source bytes",
                });
            }
        }
        if preceding_ids.contains(&input.identity.grfid) {
            return Err(Session::unsupported(
                LoadLocation { file, ..location },
                8,
                "configured identity aliases unexecuted registry prefix",
            ));
        }
        size = size
            .saturating_add(input.bytes.map_or(0, <[u8]>::len))
            .saturating_add(input.parameters.len().saturating_mul(4))
            .saturating_add(input.name.len());
        if size > options.max_source_bytes {
            return Err(ControlLoadError::ResourceLimit {
                location: LoadLocation { file, ..location },
                resource: "source/config bytes",
            });
        }
        if let Some(bytes) = input.bytes {
            GrfContainer::parse(bytes)
                .map_err(|source| ControlLoadError::Structural { file, source })?;
        }
    }
    let mut session = Session {
        inputs,
        preceding_ids,
        options,
        registry: Registry::new(inputs),
        budget: Budget::new(options),
        overrides: BTreeMap::new(),
        events: Vec::new(),
    };
    for stage in [
        LoadStage::LabelScan,
        LoadStage::Init,
        LoadStage::Reserve,
        LoadStage::Activation,
    ] {
        session.phase(stage)?;
    }
    session.overrides.clear();
    session
        .budget
        .payload(session.registry.snapshot_bytes(), location)?;
    Ok(ControlLoadReport {
        files: session.registry.snapshots(),
        events: session.events,
    })
}
