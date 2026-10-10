use super::{
    load::{ActionResult, Session},
    load_types::{ControlLoadError, LoadEvent, LoadFailure, LoadLocation, LoadStatus},
    records::Reader,
};

fn arithmetic(
    operation: u8,
    left: u32,
    right: u32,
    location: LoadLocation,
) -> Result<Option<u32>, ControlLoadError> {
    let invalid = |detail| ControlLoadError::InvalidNativeDomain { location, detail };
    let signed_left = i32::from_ne_bytes(left.to_ne_bytes());
    let signed_right = i32::from_ne_bytes(right.to_ne_bytes());
    let value = match operation {
        0 => left,
        1 => left.wrapping_add(right),
        2 => left.wrapping_sub(right),
        3 | 4 => left.wrapping_mul(right),
        5 | 6 => {
            if signed_right < 0 {
                let count = signed_right.unsigned_abs();
                if count >= 32 {
                    return Err(invalid(
                        "right shift magnitude exceeds native defined domain",
                    ));
                }
                if operation == 5 {
                    left >> count
                } else {
                    u32::from_ne_bytes((signed_left >> count).to_ne_bytes())
                }
            } else {
                left.wrapping_shl(right & 0x1f)
            }
        }
        7 => left & right,
        8 => left | right,
        9 => left.checked_div(right).unwrap_or(left),
        10 => {
            if right == 0 {
                left
            } else {
                u32::from_ne_bytes(
                    signed_left
                        .checked_div(signed_right)
                        .ok_or_else(|| invalid("signed division overflow"))?
                        .to_ne_bytes(),
                )
            }
        }
        11 => left.checked_rem(right).unwrap_or(left),
        12 => {
            if right == 0 {
                left
            } else {
                u32::from_ne_bytes(
                    signed_left
                        .checked_rem(signed_right)
                        .ok_or_else(|| invalid("signed remainder overflow"))?
                        .to_ne_bytes(),
                )
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(value))
}

impl Session<'_, '_> {
    pub(super) fn external_parameter(
        &mut self,
        source: u8,
        grfid: u32,
        location: LoadLocation,
    ) -> Result<u32, ControlLoadError> {
        self.check_preceding(grfid, u32::MAX, location, 0x0d)?;
        let config = self.registry.config_by_id(grfid, u32::MAX);
        if let Some(target) = config {
            let is_static = self.inputs.get(target).is_some_and(|i| i.flags.is_static);
            let caller_static = self
                .inputs
                .get(location.file)
                .is_some_and(|i| i.flags.is_static);
            if self.options.networking && is_static && !caller_static {
                self.disable(target, location, Some(LoadFailure::StaticInfluence))?;
                return Ok(0);
            }
            if self.registry.status(target) == LoadStatus::Disabled {
                return Ok(0);
            }
            let Some(file) = self.registry.file_by_id(grfid) else {
                return Ok(0);
            };
            if source == 0xfe {
                return Ok(self.inputs.get(target).map_or(0, |i| i.metadata_version));
            }
            return Ok(file
                .parameters
                .get(usize::from(source))
                .copied()
                .unwrap_or(0));
        }
        Ok(0)
    }
    pub(super) fn set_parameter(
        &mut self,
        reader: &mut Reader<'_>,
        location: LoadLocation,
    ) -> ActionResult {
        let target = reader.byte()?;
        let mut operation = reader.byte()?;
        let source1 = reader.byte()?;
        let source2 = reader.byte()?;
        let data = if reader.remaining() >= 4 {
            reader.dword()?
        } else {
            0
        };
        if operation & 0x80 != 0 {
            if target < 0x80
                && self
                    .registry
                    .file(location.file)
                    .is_some_and(|f| usize::from(target) < f.parameters.len())
            {
                return Ok(());
            }
            operation &= 0x7f;
        }
        let (left, right) = if source2 == 0xfe {
            if data & 0xff == 0xff && !(data == 0xffff && self.environment.is_some()) {
                return Err(Self::unsupported(
                    location,
                    0x0d,
                    "patch variables and GRM require control-2",
                )
                .into());
            }
            (
                if data == 0xffff {
                    self.environment
                        .as_ref()
                        .map_or(0, |environment| environment.patch_variable(source1))
                } else {
                    self.external_parameter(source1, data, location)?
                },
                u32::from(source2),
            )
        } else {
            (
                if source1 == 0xff {
                    data
                } else {
                    self.parameter(source1, location, 0x0d)?
                },
                if source2 == 0xff {
                    data
                } else {
                    self.parameter(source2, location, 0x0d)?
                },
            )
        };
        let Some(value) = arithmetic(operation, left, right, location)? else {
            return Ok(());
        };
        if target >= 0x80 {
            if let Some(environment) = &mut self.environment {
                let is_static = self
                    .inputs
                    .get(location.file)
                    .is_some_and(|input| input.flags.is_static);
                if let Some(file) = self.registry.file_mut(location.file) {
                    environment.target(target, value, is_static, &mut file.globals);
                }
                return Ok(());
            }
            return Err(Self::unsupported(
                location,
                0x0d,
                "special parameter target requires control-2",
            )
            .into());
        }
        if let Some(file) = self.registry.file_mut(location.file) {
            let index = usize::from(target);
            if file.parameters.len() <= index {
                file.parameters.resize(index.saturating_add(1), 0);
            }
            if let Some(parameter) = file.parameters.get_mut(index) {
                *parameter = value;
            }
        }
        let size = self
            .registry
            .file(location.file)
            .map_or(0, |f| f.parameters.len().saturating_mul(4));
        self.budget.trace(size, location)?;
        let values = self
            .registry
            .file(location.file)
            .map_or_else(Vec::new, |f| f.parameters.clone());
        self.events.push(LoadEvent::Parameters { location, values });
        Ok(())
    }
}
