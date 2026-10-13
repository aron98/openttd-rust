//! Complete one-chain vanilla restoration, separate from single-locomotive admission.
mod cache;
mod fields;
mod geometry;
mod graph;
mod input;
mod types;
use crate::content::ContentCatalog;
pub use fields::ConsistField;
use ottd_save::{
    WireValue,
    world::{PathElement, World, WorldEdit},
};
pub use types::{ConsistCaches, ConsistError, ConsistPart, ConsistRefusal};

/// Restore exactly one empty, stopped vanilla front-headed consist on flat normal rail.
///
/// Returns per-part caches and links in pool order. Saved `PoweredWagon` flags are cleared
/// for all parts and front acceleration is rebuilt in one transaction; saved capacities
/// remain unchanged with no vanilla GRF diagnostic. No movement or other world caches
/// are enabled. The front braking result is a derived getter, not a saved/cache field.
///
/// # Errors
/// Typed content, graph and domain failures leave the complete input world unchanged,
/// including failures after earlier part edits have been staged privately.
pub fn restore_consists(world: &mut World) -> Result<ConsistCaches, ConsistError> {
    let content = ContentCatalog::from_world(world)?;
    let (inputs, settings) = input::read(world, &content)?;
    let chain = graph::chain(&inputs)?;
    let pairs = graph::pairs(&chain)?;
    let root = chain.first().ok_or(ConsistRefusal::Roster)?;
    let mut capacity = chain
        .iter()
        .try_fold(0_u32, |total, part| {
            total.checked_add(u32::from(part.capacity))
        })
        .filter(|total| i32::try_from(*total).is_ok())
        .ok_or(ConsistRefusal::Limits)?;
    let mut transaction = world.transaction();
    let mut parts = Vec::with_capacity(chain.len());
    let mut prior_engine = u16::MAX;
    let mut previous: Option<&input::Part> = None;
    for part in &chain {
        geometry::validate(part, root, previous).map_err(|reason| ConsistError::Part {
            id: part.id,
            reason,
            staged_parts: transaction.touched().0,
        })?;
        let mut cache = cache::part(part, prior_engine, capacity);
        cache.slope_resistance = u32::from(part.spec.weight)
            .wrapping_mul(u32::from(settings.slope))
            .wrapping_mul(100);
        parts.push(ConsistPart {
            cache,
            first: root.id,
            previous: previous.map(|part| part.id),
            next: part.next,
            other_multiheaded_part: pairs.get(&part.id).copied(),
            braking_acceleration: None,
        });
        if part.subtype & 8 != 0 {
            prior_engine = part.engine;
        }
        capacity = capacity
            .checked_sub(u32::from(part.capacity))
            .ok_or(ConsistRefusal::Limits)?;
        transaction.apply(WorldEdit::Field {
            chunk: *b"VEHS",
            record: part.id.raw(),
            path: vec![
                PathElement::Field("train".into()),
                PathElement::Index(0),
                PathElement::Field("flags".into()),
            ],
            value: WireValue::Unsigned(u64::from(part.flags & !8)),
        })?;
        previous = Some(part);
    }
    let front = parts.first_mut().ok_or(ConsistRefusal::Roster)?;
    cache::aggregate(&chain, settings, &mut front.cache)?;
    front.braking_acceleration = Some(cache::stopped_acceleration(&front.cache)?);
    let acceleration = front
        .cache
        .power
        .checked_div(front.cache.weight)
        .ok_or(ConsistRefusal::Limits)?
        .wrapping_mul(4)
        .clamp(1, 255);
    transaction.apply(WorldEdit::Field {
        chunk: *b"VEHS",
        record: root.id.raw(),
        path: vec![
            PathElement::Field("train".into()),
            PathElement::Index(0),
            PathElement::Field("common".into()),
            PathElement::Index(0),
            PathElement::Field("acceleration".into()),
        ],
        value: WireValue::Unsigned(u64::from(acceleration)),
    })?;
    transaction.prepare()?.commit();
    parts.sort_by_key(|part| part.cache.id);
    Ok(ConsistCaches {
        front: root.id,
        parts,
    })
}
