use super::{CommandCost, CommandError, Plan};
use crate::world_access::{field, field_edit, unsigned};
use ottd_save::{WireValue, world::World};
pub(super) fn rename(
    world: &World,
    company: u8,
    text: &str,
    president: bool,
) -> Result<Plan, CommandError> {
    let field_name = if president { "president_name" } else { "name" };
    if text.chars().count() >= 32 {
        return Ok(Plan::empty(CommandCost::failure("CMD_ERROR")));
    }
    if !text.is_empty() {
        if let Some(table) = world.tables().get(b"PLYR") {
            for id in table.records().keys() {
                if matches!(field(world, b"PLYR", *id, field_name)?, WireValue::Bytes(name) if name == text.as_bytes())
                {
                    return Ok(Plan::empty(CommandCost::failure(
                        "STR_ERROR_NAME_MUST_BE_UNIQUE",
                    )));
                }
            }
        }
    }
    if matches!(field(world,b"GSDT",0,"name")?,WireValue::Bytes(name) if !name.is_empty()) {
        return Err(CommandError::Unsupported("company-name script events"));
    }
    if let Some(table) = world.tables().get(b"PLYR") {
        for id in table.records().keys() {
            if unsigned(world, b"PLYR", *id, "is_ai")? != 0 {
                return Err(CommandError::Unsupported("company-name AI events"));
            }
        }
    }
    let company = u32::from(company);
    let mut edits = vec![field_edit(
        *b"PLYR",
        company,
        field_name,
        WireValue::Bytes(text.as_bytes().to_vec()),
    )];
    if president
        && !text.is_empty()
        && unsigned(world, b"PLYR", company, "name_1")? == 0x6001
        && matches!(field(world, b"PLYR", company, "name")?, WireValue::Bytes(name) if name.is_empty())
    {
        let native_company =
            u8::try_from(company).map_err(|_| CommandError::Overflow("company"))?;
        let nested = rename(world, native_company, &format!("{text} Transport"), false)?;
        if nested.cost.success {
            edits.extend(nested.edits);
        }
    }
    Ok(Plan {
        cost: CommandCost::success(0, 255),
        edits,
    })
}
