use super::{
    load::{ActionResult, Session},
    load_cursor::{Cursor, Skip},
    load_types::{ControlLoadError, LoadEvent, LoadFailure, LoadLocation, LoadStage, LoadStatus},
    records::Reader,
};

impl Session<'_, '_> {
    fn condition_parameter(
        &self,
        number: u8,
        value: &mut u32,
        location: LoadLocation,
        action: u8,
    ) -> Result<u32, ControlLoadError> {
        if number == 0x85 {
            if let Some(environment) = &self.environment {
                return Ok(environment.patch_flags(value));
            }
        }
        self.parameter(number, location, action)
    }
    fn grf_condition(
        &mut self,
        action: u8,
        kind: u8,
        value: u32,
        mask: u32,
        location: LoadLocation,
    ) -> Result<Option<bool>, ControlLoadError> {
        self.check_preceding(value, mask, location, action)?;
        let mut config = self.registry.config_by_id(value, mask);
        if let Some(target) = config {
            if self.options.networking
                && self.inputs.get(target).is_some_and(|i| i.flags.is_static)
                && !self
                    .inputs
                    .get(location.file)
                    .is_some_and(|i| i.flags.is_static)
            {
                self.disable(target, location, Some(LoadFailure::StaticInfluence))?;
                config = None;
            }
        }
        if config.is_none() && kind != 10 {
            return Ok(None);
        }
        let status = config.map(|index| self.registry.status(index));
        Ok(match kind {
            6 => Some(status == Some(LoadStatus::Activated)),
            7 => Some(status != Some(LoadStatus::Activated)),
            8 => Some(status == Some(LoadStatus::Initialised)),
            9 => Some(matches!(
                status,
                Some(LoadStatus::Initialised | LoadStatus::Activated)
            )),
            10 => Some(matches!(
                status,
                None | Some(LoadStatus::Disabled | LoadStatus::NotFound)
            )),
            _ => None,
        })
    }
    pub(super) fn condition(
        &mut self,
        action: u8,
        reader: &mut Reader<'_>,
        cursor: &mut Cursor<'_>,
        location: LoadLocation,
    ) -> ActionResult {
        let parameter = reader.byte()?;
        let mut width = reader.byte()?;
        let kind = reader.byte()?;
        if kind < 2 {
            width = 1;
        }
        let (mut value, mask) = match width {
            8 => (reader.dword()?, reader.dword()?),
            4 => (reader.dword()?, u32::MAX),
            2 => (u32::from(reader.word()?), 0xffff),
            1 => (u32::from(reader.byte()?), 0xff),
            _ => (0, 0),
        };
        if parameter < 0x80
            && self
                .registry
                .file(location.file)
                .is_none_or(|file| usize::from(parameter) >= file.parameters.len())
        {
            return Ok(());
        }
        let result = if kind >= 0x0b {
            if kind <= 0x12 {
                return Err(Self::unsupported(
                    location,
                    action,
                    "catalog label predicates require control-2",
                )
                .into());
            }
            return Ok(());
        } else if parameter == 0x88 {
            self.grf_condition(action, kind, value, mask, location)?
        } else {
            let parameter = self.condition_parameter(parameter, &mut value, location, action)?;
            match kind {
                0 | 1 => {
                    if value >= 32 {
                        return Err(ControlLoadError::InvalidNativeDomain {
                            location,
                            detail: "bit test exceeds native width",
                        }
                        .into());
                    }
                    let set = parameter & (1_u32 << value) != 0;
                    Some(if kind == 0 { set } else { !set })
                }
                2 => Some(parameter & mask == value),
                3 => Some(parameter & mask != value),
                4 => Some((parameter & mask) < value),
                5 => Some((parameter & mask) > value),
                _ => None,
            }
        };
        if result != Some(true) {
            return Ok(());
        }
        let count = reader.byte()?;
        let mut chosen = None;
        if let Some(file) = self.registry.file(location.file) {
            for label in &file.labels {
                if label.id != count {
                    continue;
                }
                if chosen.is_none() {
                    chosen = Some(*label);
                }
                if label.line > location.line {
                    chosen = Some(*label);
                    break;
                }
            }
        }
        if let Some(label) = chosen {
            cursor.reader.pos = label.offset;
            cursor.line = label.line;
            self.emit(
                LoadEvent::Jump {
                    location,
                    target_line: label.line,
                    target_offset: label.offset,
                },
                0,
                location,
            )?;
        } else if count == 0 {
            cursor.skip = Skip::Stop;
            let expected = if location.stage < LoadStage::Reserve {
                LoadStatus::Initialised
            } else {
                LoadStatus::Activated
            };
            if self.registry.status(location.file) != expected {
                self.disable(location.file, location, None)?;
            }
        } else {
            cursor.skip = Skip::Count(i32::from(count));
        }
        Ok(())
    }
}
