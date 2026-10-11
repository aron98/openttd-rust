//! Native cargo packet identity, splitting and loading-coordinate operations.
use super::{CargoRuntime, RuntimeError};
use crate::{WorldTickError, WorldTickState as State, unsupported_tick as unsupported};
use ottd_save::{TableRecord, WireValue, world::WorldEdit};

pub(super) fn split(
    state: &mut State<'_>,
    cargo: &mut CargoRuntime,
    packet: u32,
    count: u16,
) -> Result<Option<u32>, WorldTickError> {
    if !cargo.pool.can_allocate(1) {
        return Ok(None);
    }
    let old_count = state.number(b"CAPA", packet, "count")?;
    if count == 0 || i64::from(count) >= old_count {
        return Err(unsupported("cargo_split", "partial nonzero count"));
    }
    let feeder = state.number(b"CAPA", packet, "feeder_share")?;
    let share = feeder
        .wrapping_mul(i64::from(count))
        .checked_div(old_count)
        .ok_or_else(|| unsupported("cargo_split", "feeder divisor"))?;
    let id = cargo.pool.allocate().map_err(RuntimeError::from)?;
    let table = state.table(b"CAPA")?;
    let original = table
        .record(packet)
        .ok_or_else(|| unsupported("cargo_split", "packet"))?;
    let mut values = original.values().to_vec();
    for (field, value) in table.schema().fields().iter().zip(&mut values) {
        match field.name() {
            "count" => *value = WireValue::Unsigned(u64::from(count)),
            "feeder_share" => *value = WireValue::Signed(share),
            _ => {}
        }
    }
    state.apply(WorldEdit::InsertRecord {
        chunk: *b"CAPA",
        record: id,
        value: TableRecord::new(values),
    })?;
    state.set_number(
        b"CAPA",
        packet,
        "count",
        old_count.wrapping_sub(i64::from(count)),
    )?;
    state.set_number(b"CAPA", packet, "feeder_share", feeder.wrapping_sub(share))?;
    Ok(Some(id))
}

pub(super) fn loading_tile(
    state: &mut State<'_>,
    packet: u32,
    tile: u32,
) -> Result<(), WorldTickError> {
    if state.number(b"CAPA", packet, "source_xy")? == i64::from(u32::MAX) {
        state.set_number(b"CAPA", packet, "source_xy", i64::from(tile))?;
    }
    let width = std::num::NonZeroU32::new(state.width)
        .ok_or_else(|| unsupported("cargo_load", "map width"))?;
    for (name, coordinate) in [("travelled.x", tile % width), ("travelled.y", tile / width)] {
        let old = i16::try_from(state.number(b"CAPA", packet, name)?)
            .map_err(|_| unsupported("cargo_load", "travelled coordinate"))?;
        let coordinate =
            i16::try_from(coordinate).map_err(|_| unsupported("cargo_load", "tile coordinate"))?;
        state.set_number(
            b"CAPA",
            packet,
            name,
            i64::from(old.wrapping_add(coordinate)),
        )?;
    }
    Ok(())
}

pub(super) fn merge(
    state: &mut State<'_>,
    cargo: &mut CargoRuntime,
    into: u32,
    from: u32,
) -> Result<bool, WorldTickError> {
    if source(state, into)? != source(state, from)? {
        return Ok(false);
    }
    for name in ["source_xy", "periods_in_transit", "source"] {
        if state.number(b"CAPA", into, name)? != state.number(b"CAPA", from, name)? {
            return Ok(false);
        }
    }
    let count = state
        .number(b"CAPA", into, "count")?
        .wrapping_add(state.number(b"CAPA", from, "count")?);
    if count > i64::from(u16::MAX) {
        return Ok(false);
    }
    let feeder = state
        .number(b"CAPA", into, "feeder_share")?
        .wrapping_add(state.number(b"CAPA", from, "feeder_share")?);
    state.set_number(b"CAPA", into, "count", count)?;
    state.set_number(b"CAPA", into, "feeder_share", feeder)?;
    state.apply(WorldEdit::RemoveRecord {
        chunk: *b"CAPA",
        record: from,
    })?;
    cargo.pool.free(from).map_err(RuntimeError::from)?;
    Ok(true)
}

fn source(state: &State<'_>, id: u32) -> Result<crate::cargo::CargoSource, WorldTickError> {
    let source = u16::try_from(state.number(b"CAPA", id, "source_id")?)
        .map_err(|_| unsupported("cargo_packet", "source id"))?;
    match state.number(b"CAPA", id, "source_type")? {
        0 => Ok(crate::cargo::CargoSource::Industry(source)),
        1 => Ok(crate::cargo::CargoSource::Town(source)),
        _ => Err(unsupported("cargo_packet", "producer source type")),
    }
}
pub(super) fn reduce(state: &mut State<'_>, id: u32, removed: u32) -> Result<(), WorldTickError> {
    let count = state.number(b"CAPA", id, "count")?;
    let feeder = state.number(b"CAPA", id, "feeder_share")?;
    let share = feeder
        .wrapping_mul(i64::from(removed))
        .checked_div(count)
        .ok_or_else(|| unsupported("cargo_packet", "count divisor"))?;
    state.set_number(b"CAPA", id, "count", count.wrapping_sub(i64::from(removed)))?;
    state.set_number(b"CAPA", id, "feeder_share", feeder.wrapping_sub(share))
}
