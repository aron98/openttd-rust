use super::{
    action14_parameters::{info_binary, parameter_binary},
    metadata_types::{GrfStaticInfo, ParameterInfo, TextList},
    records::Reader,
    text::{Budget, localized},
    types::ScanError,
};

#[derive(Clone, Copy)]
enum Context {
    Root,
    Info,
    Parameters,
    Parameter(usize),
    Values(usize),
    Unknown,
}

fn child(context: Context, id: [u8; 4], info: &mut GrfStaticInfo<'_>) -> Context {
    match (context, &id) {
        (Context::Root, b"INFO") => Context::Info,
        (Context::Info, b"PARA") => Context::Parameters,
        (Context::Parameter(index), b"VALU") => Context::Values(index),
        (Context::Parameters, _) => {
            let id = u32::from_le_bytes(id);
            if id >= u32::from(info.num_valid_params) {
                return Context::Unknown;
            }
            let Ok(slot) = u8::try_from(id) else {
                return Context::Unknown;
            };
            let index = usize::from(slot);
            if info.parameters.len() <= index {
                info.parameters
                    .resize_with(index.saturating_add(1), || None);
            }
            if let Some(parameter) = info.parameters.get_mut(index) {
                parameter.get_or_insert_with(|| ParameterInfo::new(slot));
            }
            Context::Parameter(index)
        }
        _ => Context::Unknown,
    }
}

const fn known_binary(context: Context, id: [u8; 4]) -> bool {
    match context {
        Context::Info => matches!(&id, b"NPAR" | b"PALS" | b"BLTR" | b"VRSN" | b"MINV"),
        Context::Parameter(_) => matches!(&id, b"TYPE" | b"LIMI" | b"MASK" | b"DFLT"),
        Context::Root | Context::Parameters | Context::Values(_) | Context::Unknown => false,
    }
}

fn text_target<'b, 'a>(
    info: &'b mut GrfStaticInfo<'a>,
    context: Context,
    id: [u8; 4],
) -> Option<&'b mut TextList<'a>> {
    match context {
        Context::Info => match &id {
            b"NAME" => Some(&mut info.name),
            b"DESC" => Some(&mut info.description),
            b"URL_" => Some(&mut info.url),
            _ => None,
        },
        Context::Parameter(index) => {
            let parameter = info.parameters.get_mut(index)?.as_mut()?;
            match &id {
                b"NAME" => Some(&mut parameter.name),
                b"DESC" => Some(&mut parameter.description),
                _ => None,
            }
        }
        Context::Values(index) => {
            let parameter = info.parameters.get_mut(index)?.as_mut()?;
            let value = u32::from_le_bytes(id);
            if value > parameter.max {
                return None;
            }
            let index = match parameter
                .value_names
                .binary_search_by_key(&value, |(value, _)| *value)
            {
                Ok(index) => index,
                Err(index) => {
                    parameter
                        .value_names
                        .insert(index, (value, TextList::default()));
                    index
                }
            };
            parameter.value_names.get_mut(index).map(|(_, text)| text)
        }
        Context::Root | Context::Parameters | Context::Unknown => None,
    }
}

pub(super) fn parse<'a>(
    reader: &mut Reader<'a>,
    info: &mut GrfStaticInfo<'a>,
    budget: &mut Budget,
    source: usize,
) -> Result<(), ScanError> {
    let mut stack = vec![Context::Root];
    while let Some(context) = stack.last().copied() {
        let kind = reader.byte()?;
        if kind == 0 {
            stack.pop();
            continue;
        }
        budget.node(source.saturating_add(reader.pos))?;
        let id = reader.dword()?.to_le_bytes();
        match kind {
            b'C' => {
                budget.depth(stack.len(), source.saturating_add(reader.pos))?;
                stack.push(child(context, id, info));
            }
            b'B' => {
                let len = usize::from(reader.word()?);
                let known = known_binary(context, id);
                if known && len > reader.remaining() {
                    return Ok(());
                }
                let bytes = reader.take(len)?;
                match context {
                    Context::Info if known => info_binary(info, id, bytes)?,
                    Context::Parameter(index) if known => {
                        if let Some(parameter) =
                            info.parameters.get_mut(index).and_then(Option::as_mut)
                        {
                            parameter_binary(parameter, id, bytes)?;
                        }
                        if &id == b"DFLT" {
                            info.has_param_defaults = true;
                        }
                    }
                    _ => (),
                }
            }
            b'T' => {
                let language = reader.byte()?;
                let raw = reader.string()?;
                if let Some(target) = text_target(info, context, id) {
                    target.insert(localized(
                        raw,
                        language,
                        0,
                        &id == b"DESC" && !matches!(context, Context::Values(_)),
                        budget,
                        source.saturating_add(reader.pos),
                    )?);
                }
            }
            _ => return Ok(()),
        }
    }
    Ok(())
}
