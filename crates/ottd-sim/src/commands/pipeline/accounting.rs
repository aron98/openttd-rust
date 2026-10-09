use super::{CommandCost, CommandError, TerrainRead};
use crate::world_access::field_edit;
use ottd_save::{
    WireValue,
    world::{PathElement, WorldEdit},
};

pub(in crate::commands) fn completion_edits(
    world: TerrainRead<'_>,
    company: u32,
    tile: u32,
    result: &CommandCost,
) -> Result<Vec<WorldEdit>, CommandError> {
    let pause = world.unsigned(*b"DATE", 0, "pause_mode")?;
    let mut edits = Vec::new();
    if tile != 0 {
        edits.push(field_edit(
            *b"PLYR",
            company,
            "last_build_coordinate",
            WireValue::Unsigned(u64::from(tile)),
        ));
    }
    if pause != 0 {
        edits.push(field_edit(
            *b"DATE",
            0,
            "pause_mode",
            WireValue::Unsigned(pause | 128),
        ));
    }
    if result.cost != 0 {
        let money = world
            .signed(*b"PLYR", company, "money")?
            .saturating_sub(result.cost);
        edits.push(field_edit(
            *b"PLYR",
            company,
            "money",
            WireValue::Signed(money),
        ));
        let WireValue::Array(expenses) = world.field(*b"PLYR", company, "yearly_expenses")? else {
            return Err(CommandError::Unsupported("yearly expense wire layout"));
        };
        let index = usize::from(result.expenses);
        let Some(WireValue::Signed(expense)) = expenses.get(index) else {
            return Err(CommandError::Unsupported("expense category"));
        };
        edits.push(WorldEdit::Field {
            chunk: *b"PLYR",
            record: company,
            path: vec![
                PathElement::Field("yearly_expenses".into()),
                PathElement::Index(index),
            ],
            value: WireValue::Signed(expense.saturating_add(result.cost)),
        });
    }
    Ok(edits)
}
