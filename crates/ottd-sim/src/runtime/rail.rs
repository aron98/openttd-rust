//! Isolated vanilla single-locomotive after-load boundary; no train ticking.
mod cache;
mod record;
use crate::content::ContentCatalog;
pub use cache::RailVehicleCache;
use ottd_save::{
    WireValue,
    world::{PathElement, World, WorldEdit},
};

/// Supported-domain refusal before any saved-state publication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RailRefusal {
    /// Requires exactly one train and no other vehicles.
    #[error("requires one train")]
    VehicleRoster,
    /// Multihead, articulated, wagon and linked consists need additional restoration.
    #[error("unsupported train consist")]
    Consist,
    /// Orders and backups require the full runtime load context.
    #[error("unsupported train order context")]
    Orders,
    /// Populated cargo needs cargo-cache restoration.
    #[error("requires empty train cargo")]
    Cargo,
    /// This boundary admits stopped, zero-speed locomotives only.
    #[error("requires stopped locomotive")]
    Motion,
    /// Only vanilla steam/diesel singleheads with no original capacity are admitted.
    #[error("unsupported locomotive specification")]
    Engine,
    /// Only matching straight plain normal rail is admitted.
    #[error("unsupported train track context")]
    Track,
}

/// An invalid or unsupported train after-load input.
#[derive(Debug, thiserror::Error)]
pub enum RailRestoreError {
    /// A structurally valid feature outside this boundary.
    #[error(transparent)]
    Unsupported(#[from] RailRefusal),
    /// Missing, malformed or incorrectly sized saved value.
    #[error("invalid train saved field: {0}")]
    Invalid(&'static str),
    /// Non-vanilla or malformed content configuration.
    #[error(transparent)]
    Content(#[from] crate::content::ContentError),
    /// Saved transaction could not be prepared; the input remains unchanged.
    #[error(transparent)]
    World(#[from] ottd_save::world::WorldError),
}

/// Restore one empty vanilla locomotive's native train caches and saved after-load edits.
///
/// The borrowed world is changed only after all admission, calculation and transaction
/// validation succeeds. This does not restore unrelated world/runtime caches or enable ticks.
/// Vanilla capacity mismatches retain the saved capacity without a diagnostic, matching
/// `ShowNewGrfVehicleError`'s missing-GRF return.
///
/// # Errors
/// Rejects unsupported consists, cargo, orders, motion, engines and track context;
/// malformed saved fields and failed transactions leave the complete world unchanged.
pub fn restore(world: &mut World) -> Result<RailVehicleCache, RailRestoreError> {
    let content = ContentCatalog::from_world(world)?;
    let input = record::read(world, &content)?;
    let cache = cache::restore(&input)?;
    let ratio = cache
        .power
        .checked_div(cache.weight)
        .ok_or(RailRestoreError::Invalid("weight"))?;
    let acceleration = u8::try_from(ratio.wrapping_mul(4).clamp(1, 255))
        .map_err(|_| RailRestoreError::Invalid("acceleration"))?;
    let mut transaction = world.transaction();
    for (common, name, value) in [
        (true, "acceleration", u64::from(acceleration)),
        (false, "flags", u64::from(input.flags & !8)),
    ] {
        let mut path = vec![PathElement::Field("train".into()), PathElement::Index(0)];
        if common {
            path.extend([PathElement::Field("common".into()), PathElement::Index(0)]);
        }
        path.push(PathElement::Field(name.into()));
        transaction.apply(WorldEdit::Field {
            chunk: *b"VEHS",
            record: input.id.raw(),
            path,
            value: WireValue::Unsigned(value),
        })?;
    }
    transaction.prepare()?.commit();
    Ok(cache)
}
