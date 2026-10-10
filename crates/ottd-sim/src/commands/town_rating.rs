#[cfg(test)]
use super::terrain_context::trace::Event;
use super::{
    CommandError,
    terrain_context::{ClearRequest, TerrainContext},
    terrain_read::TerrainRead,
    terrain_state::TerrainState,
};
use crate::world_access::{WorldAccessError, field_edit, row_field};
use ottd_save::{
    WireValue,
    world::{PathElement, WorldEdit},
};

pub(super) fn nearest(view: TerrainRead<'_>, tile: u32) -> Result<Option<u16>, CommandError> {
    let width = std::num::NonZeroU32::new(view.size().width())
        .ok_or(CommandError::Overflow("map width"))?;
    let threshold = u8::try_from(view.unsigned(*b"PATS", 0, "economy.dist_local_authority")?)
        .map_err(|_| CommandError::Overflow("authority distance"))?;
    if !(5..=60).contains(&threshold) {
        return Err(CommandError::Unsupported(
            "noncanonical town authority distance",
        ));
    }
    let mut best = None;
    view.visit_towns(|id, schema, row| {
        let WireValue::Unsigned(centre) = row_field(schema, row, "xy")? else {
            return Err(WorldAccessError("CITY/xy".into()).into());
        };
        let centre = u32::try_from(*centre).map_err(|_| CommandError::Overflow("town tile"))?;
        if centre >= view.size().count()? {
            return Err(CommandError::Unsupported("town outside map"));
        }
        let distance = (tile % width)
            .abs_diff(centre % width)
            .checked_add((tile / width).abs_diff(centre / width))
            .ok_or(CommandError::Overflow("town distance"))?;
        let id = u16::try_from(id).map_err(|_| CommandError::Overflow("town identity"))?;
        let candidate = (distance, id);
        if best.is_none_or(|old| candidate < old) {
            best = Some(candidate);
        }
        Ok(())
    })?;
    Ok(best
        .filter(|(distance, _)| *distance < u32::from(threshold))
        .map(|(_, id)| id))
}
fn saved_rating(view: TerrainRead<'_>, town: u16, company: u8) -> Result<i32, CommandError> {
    let WireValue::Array(ratings) = view.field(*b"CITY", u32::from(town), "ratings")? else {
        return Err(WorldAccessError("CITY/ratings".into()).into());
    };
    let Some(WireValue::Signed(value)) = ratings.get(usize::from(company)) else {
        return Err(WorldAccessError("CITY/ratings/company".into()).into());
    };
    Ok(i32::from(
        i16::try_from(*value).map_err(|_| CommandError::Overflow("town rating"))?,
    ))
}
pub(super) fn clear_tree(
    state: &mut TerrainState<'_, '_>,
    context: &mut TerrainContext,
    request: ClearRequest,
) -> Result<(), CommandError> {
    let view = state.read();
    if !view.has_record(*b"PLYR", u32::from(context.company)) {
        return Ok(());
    }
    let Some(town) = nearest(view, request.tile)? else {
        return Ok(());
    };
    if request.flags.suppress_rating() || view.signed(*b"CHTS", 0, "magic_bulldozer.value")? != 0 {
        #[cfg(test)]
        context.record(Event::Suppressed {
            town,
            flags: request.flags.0,
            company: context.company,
            test_mode: context.testing(),
        });
        return Ok(());
    }
    let saved = saved_rating(view, town, context.company)?;
    let before = if context.testing() {
        context.ratings.get(&town).copied().unwrap_or(saved)
    } else {
        saved
    };
    let after = if before > -1000 {
        before
            .checked_sub(35)
            .ok_or(CommandError::Overflow("tree rating penalty"))?
            .max(-1000)
    } else {
        before
    };
    if context.testing() {
        context.ratings.insert(town, after);
    } else {
        let have = u16::try_from(view.unsigned(*b"CITY", u32::from(town), "have_ratings")?)
            .map_err(|_| CommandError::Overflow("town rating companies"))?;
        let company_bit = 1_u16
            .checked_shl(u32::from(context.company))
            .ok_or(CommandError::Overflow("town rating company"))?;
        state.apply(vec![
            WorldEdit::Field {
                chunk: *b"CITY",
                record: u32::from(town),
                path: vec![
                    PathElement::Field("ratings".into()),
                    PathElement::Index(usize::from(context.company)),
                ],
                value: WireValue::Signed(i64::from(after)),
            },
            field_edit(
                *b"CITY",
                u32::from(town),
                "have_ratings",
                WireValue::Unsigned(u64::from(have | company_bit)),
            ),
        ])?;
    }
    #[cfg(test)]
    context.record(Event::Applied {
        town,
        flags: request.flags.0,
        company: context.company,
        test_mode: context.testing(),
        before_rating: before,
        after_rating: after,
        saved_rating: saved_rating(state.read(), town, context.company)?,
        have_ratings: u16::try_from(state.read().unsigned(
            *b"CITY",
            u32::from(town),
            "have_ratings",
        )?)
        .map_err(|_| CommandError::Overflow("town rating companies"))?,
    });
    Ok(())
}
