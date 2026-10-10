//! Vanilla `AirportSpec` depot offsets and `Airport::GetRotatedTileFromOffset`, pin14ec.
use super::{
    RuntimeError,
    view::{OrderReader, Row},
};
pub(super) struct Layout {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) depots: &'static [(u32, u32)],
}
pub(super) const fn layout(kind: u8) -> Result<Layout, RuntimeError> {
    let (width, height, depots): (u32, u32, &'static [(u32, u32)]) = match kind {
        0 => (4, 3, &[(3, 0)]),
        1 | 3 => (6, 6, &[(5, 0)]),
        2 | 9 => (1, 1, &[]),
        4 => (7, 7, &[(0, 3), (6, 1)]),
        5 => (5, 4, &[(4, 0)]),
        6 => (2, 2, &[(1, 0)]),
        7 => (9, 11, &[(0, 5), (8, 4)]),
        8 => (4, 2, &[(0, 0)]),
        _ => return Err(RuntimeError::Unsupported("custom airport specification")),
    };
    Ok(Layout {
        width,
        height,
        depots,
    })
}
pub(super) fn hangar_tiles(
    kind: u8,
    rotation: u8,
    origin: u32,
    width: u32,
    height: u32,
) -> Result<Vec<u32>, RuntimeError> {
    let spec = layout(kind)?;
    if width == 0 || height == 0 {
        return Err(RuntimeError::Invalid("airport map dimensions"));
    }
    let x = origin
        .checked_rem(width)
        .ok_or(RuntimeError::Invalid("airport origin"))?;
    let y = origin
        .checked_div(width)
        .ok_or(RuntimeError::Invalid("airport origin"))?;
    let (w, h) = match rotation {
        0 | 4 => (spec.width, spec.height),
        2 | 6 => (spec.height, spec.width),
        _ => return Err(RuntimeError::Invalid("airport rotation")),
    };
    if x.checked_add(w).is_none_or(|v| v > width) || y.checked_add(h).is_none_or(|v| v > height) {
        return Err(RuntimeError::Invalid("airport extent"));
    }
    spec.depots
        .iter()
        .map(|(dx, dy)| {
            let (rx, ry) = match rotation {
                0 => (*dx, *dy),
                2 => (*dy, spec.width.saturating_sub(1).saturating_sub(*dx)),
                4 => (
                    spec.width.saturating_sub(1).saturating_sub(*dx),
                    spec.height.saturating_sub(1).saturating_sub(*dy),
                ),
                6 => (spec.height.saturating_sub(1).saturating_sub(*dy), *dx),
                _ => return Err(RuntimeError::Invalid("airport rotation")),
            };
            ry.checked_mul(width)
                .and_then(|v| v.checked_add(rx))
                .and_then(|v| v.checked_add(origin))
                .ok_or(RuntimeError::Invalid("airport tile overflow"))
        })
        .collect()
}
pub(super) fn is_hangar(
    reader: OrderReader<'_>,
    tile: u32,
    width: u32,
    height: u32,
) -> Result<bool, RuntimeError> {
    let t = reader.tile(tile)?;
    if t.tile_type() >> 4 != 5 || (t.m6() >> 3) & 15 != 1 {
        return Ok(false);
    }
    // The caller's vanilla content admission excludes overrides of built-in IDs.
    let rows = reader.rows(*b"STNN")?;
    let station = rows
        .iter()
        .find(|(id, _)| *id == u32::from(t.m2()))
        .ok_or(RuntimeError::Invalid("airport station"))?
        .1;
    let (schema, rows) = station.children("normal")?;
    let [record] = rows else {
        return Err(RuntimeError::Invalid("airport station variant"));
    };
    let station = Row { schema, record };
    let kind = u8::try_from(station.number("airport.type")?)
        .map_err(|_| RuntimeError::Invalid("airport type"))?;
    let rotation = u8::try_from(station.number("airport.rotation")?)
        .map_err(|_| RuntimeError::Invalid("airport rotation"))?;
    let origin = u32::try_from(station.number("airport.tile")?)
        .map_err(|_| RuntimeError::Invalid("airport origin"))?;
    Ok(hangar_tiles(kind, rotation, origin, width, height)?.contains(&tile))
}
