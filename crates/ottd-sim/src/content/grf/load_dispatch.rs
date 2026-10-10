use super::{
    load::{ActionResult, Session},
    load_cursor::{Cursor, Skip, count_records},
    load_types::{LoadEvent, LoadFailure, LoadLabel, LoadLocation, LoadStage, LoadStatus},
    records::Reader,
};

impl Session<'_, '_> {
    pub(super) fn dispatch(
        &mut self,
        reader: &mut Reader<'_>,
        cursor: &mut Cursor<'_>,
        location: LoadLocation,
    ) -> ActionResult {
        let action = reader.byte()?;
        if location.stage == LoadStage::LabelScan {
            match action {
                0x10 => {
                    let label = reader.byte()?;
                    self.budget.label(location)?;
                    if let Some(file) = self.registry.file_mut(location.file) {
                        file.labels.push(LoadLabel {
                            id: label,
                            line: location.line,
                            offset: cursor.reader.pos,
                        });
                    }
                    self.emit(
                        LoadEvent::Label {
                            location,
                            label,
                            post_record_offset: cursor.reader.pos,
                        },
                        0,
                        location,
                    )?;
                }
                1 | 5 | 0x0a | 0x11 | 0x12 => {
                    let count = count_records(action, reader)?;
                    cursor.skip = Skip::from_count(count);
                }
                _ => (),
            }
            return Ok(());
        }
        if ignored(action, location.stage) {
            return Ok(());
        }
        match action {
            0 if self.language.is_some() => self.language_properties(reader, location)?,
            4 if self.language.is_some() => self.generic_names(reader, location)?,
            0x13 if self.language.is_some() => self.translate_strings(reader, cursor, location)?,
            6 => self.substitute(reader, cursor, location)?,
            7 | 9 => self.condition(action, reader, cursor, location)?,
            8 => self.info(reader, cursor, location)?,
            0x0d => self.set_parameter(reader, location)?,
            1 | 5 | 0x0a | 0x11 | 0x12
                if location.stage != LoadStage::Activation
                    && (action != 0x11 || location.stage == LoadStage::Reserve) =>
            {
                let count = count_records(action, reader)?;
                cursor.skip = Skip::from_count(count);
            }
            0..=5 | 0x0a | 0x0b | 0x0e | 0x0f | 0x11..=0x13 => {
                return Err(Self::unsupported(
                    location,
                    action,
                    "executed handler outside control-1",
                )
                .into());
            }
            0x0c | 0x10 | 0x14..=255 => (),
        }
        Ok(())
    }
    fn info(
        &mut self,
        reader: &mut Reader<'_>,
        cursor: &mut Cursor<'_>,
        location: LoadLocation,
    ) -> ActionResult {
        let version = reader.byte()?;
        let grfid = reader.dword()?;
        reader.string()?;
        if location.stage < LoadStage::Reserve
            && self.registry.status(location.file) != LoadStatus::Unknown
        {
            self.disable(location.file, location, Some(LoadFailure::MultipleAction8))?;
            cursor.skip = Skip::Stop;
            return Ok(());
        }
        if let Some(file) = self.registry.file_mut(location.file) {
            file.grfid = grfid;
            file.version = version;
        }
        let status = if location.stage < LoadStage::Reserve {
            LoadStatus::Initialised
        } else {
            LoadStatus::Activated
        };
        if let Some(config) = self.registry.configs.get_mut(location.file) {
            config.status = status;
        }
        self.emit(
            LoadEvent::Status {
                location,
                target_file: location.file,
                status,
            },
            0,
            location,
        )?;
        Ok(())
    }
}

const fn ignored(action: u8, stage: LoadStage) -> bool {
    match stage {
        LoadStage::Init => matches!(action, 0 | 2 | 3 | 4 | 7 | 0x13),
        LoadStage::Reserve => matches!(action, 2 | 3 | 4 | 0x0f | 0x13),
        LoadStage::Activation => action == 0x0f,
        LoadStage::FileScan | LoadStage::SafetyScan | LoadStage::LabelScan => false,
    }
}
