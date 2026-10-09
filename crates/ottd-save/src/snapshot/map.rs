use super::{SnapshotError, invalid, required, schema, table};
use crate::{ChunkKind, Reader, Savegame};
use serde::{Deserialize, Serialize};

/// All raw tile fields, before world-level validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileRawParts {
    /// Type nibble and flags.
    pub tile_type: u8,
    /// Height.
    pub height: u8,
    /// Raw field.
    pub m1: u8,
    /// Raw field.
    pub m2: u16,
    /// Raw field.
    pub m3: u8,
    /// Raw field.
    pub m4: u8,
    /// Raw field.
    pub m5: u8,
    /// Raw field.
    pub m6: u8,
    /// Raw field.
    pub m7: u8,
    /// Raw field.
    pub m8: u16,
}

impl From<TileRawParts> for TileState {
    fn from(p: TileRawParts) -> Self {
        Self {
            tile_type: p.tile_type,
            height: p.height,
            m1: p.m1,
            m2: p.m2,
            m3: p.m3,
            m4: p.m4,
            m5: p.m5,
            m6: p.m6,
            m7: p.m7,
            m8: p.m8,
        }
    }
}
impl From<&TileState> for TileRawParts {
    fn from(p: &TileState) -> Self {
        Self {
            tile_type: p.tile_type,
            height: p.height,
            m1: p.m1,
            m2: p.m2,
            m3: p.m3,
            m4: p.m4,
            m5: p.m5,
            m6: p.m6,
            m7: p.m7,
            m8: p.m8,
        }
    }
}

/// Raw tile bytes, preserving every bit of the ten saved tile planes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TileState {
    #[serde(rename = "type")]
    tile_type: u8,
    height: u8,
    m1: u8,
    m2: u16,
    m3: u8,
    m4: u8,
    m5: u8,
    m6: u8,
    m7: u8,
    m8: u16,
}

impl TileState {
    /// Raw `tile_type` tile field.
    pub const fn tile_type(&self) -> u8 {
        self.tile_type
    }
    /// Raw `height` tile field.
    pub const fn height(&self) -> u8 {
        self.height
    }
    /// Raw `m1` tile field.
    pub const fn m1(&self) -> u8 {
        self.m1
    }
    /// Raw `m2` tile field.
    pub const fn m2(&self) -> u16 {
        self.m2
    }
    /// Raw `m3` tile field.
    pub const fn m3(&self) -> u8 {
        self.m3
    }
    /// Raw `m4` tile field.
    pub const fn m4(&self) -> u8 {
        self.m4
    }
    /// Raw `m5` tile field.
    pub const fn m5(&self) -> u8 {
        self.m5
    }
    /// Raw `m6` tile field.
    pub const fn m6(&self) -> u8 {
        self.m6
    }
    /// Raw `m7` tile field.
    pub const fn m7(&self) -> u8 {
        self.m7
    }
    /// Raw `m8` tile field.
    pub const fn m8(&self) -> u16 {
        self.m8
    }
}

/// Map dimensions and tiles in linear upstream index order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, try_from = "MapWire")]
pub struct MapState {
    width: u32,
    height: u32,
    tiles: Vec<TileState>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapWire {
    width: u32,
    height: u32,
    tiles: Vec<TileState>,
}

impl TryFrom<MapWire> for MapState {
    type Error = SnapshotError;
    fn try_from(raw: MapWire) -> Result<Self, Self::Error> {
        if tile_count(raw.width, raw.height)? != raw.tiles.len() {
            return Err(invalid("tile count differs from map dimensions"));
        }
        Ok(Self {
            width: raw.width,
            height: raw.height,
            tiles: raw.tiles,
        })
    }
}

impl MapState {
    /// Width in tiles.
    pub const fn width(&self) -> u32 {
        self.width
    }
    /// Height in tiles.
    pub const fn height(&self) -> u32 {
        self.height
    }
    /// Raw tiles in linear index order.
    pub fn tiles(&self) -> &[TileState] {
        &self.tiles
    }
}

fn tile_count(width: u32, height: u32) -> Result<usize, SnapshotError> {
    if [width, height]
        .iter()
        .any(|side| !(64..=4096).contains(side) || !side.is_power_of_two())
    {
        return Err(invalid("map dimensions must be powers of two in 64..=4096"));
    }
    usize::try_from(
        width
            .checked_mul(height)
            .ok_or_else(|| invalid("map size overflow"))?,
    )
    .map_err(|_| invalid("map size overflow"))
}

pub(super) fn decode(save: &Savegame) -> Result<MapState, SnapshotError> {
    let fields = table::single(required(save, *b"MAPS")?, schema::MAP)?;
    let width = table::unsigned(&fields, "dim_x")?;
    let height = table::unsigned(&fields, "dim_y")?;
    let count = tile_count(width, height)?;
    let mut planes = Vec::new();
    for (id, bytes) in [
        (b"MAPT", 1),
        (b"MAPH", 1),
        (b"MAPO", 1),
        (b"MAP2", 2),
        (b"M3LO", 1),
        (b"M3HI", 1),
        (b"MAP5", 1),
        (b"MAPE", 1),
        (b"MAP7", 1),
        (b"MAP8", 2),
    ] {
        let chunk = required(save, *id)?;
        if chunk.kind() != ChunkKind::Riff || chunk.body().len() != count.saturating_mul(bytes) {
            return Err(invalid(format!(
                "invalid plane {} length or mode",
                String::from_utf8_lossy(id)
            )));
        }
        planes.push(Reader::new(chunk.body()));
    }
    let [types, heights, m1, m2, m3, m4, m5, m6, m7, m8] = planes.as_mut_slice() else {
        return Err(invalid("map plane count"));
    };
    let tiles = (0..count)
        .map(|_| {
            Ok(TileState {
                tile_type: types.byte()?,
                height: heights.byte()?,
                m1: m1.byte()?,
                m2: u16::from_be_bytes([m2.byte()?, m2.byte()?]),
                m3: m3.byte()?,
                m4: m4.byte()?,
                m5: m5.byte()?,
                m6: m6.byte()?,
                m7: m7.byte()?,
                m8: u16::from_be_bytes([m8.byte()?, m8.byte()?]),
            })
        })
        .collect::<Result<_, SnapshotError>>()?;
    Ok(MapState {
        width,
        height,
        tiles,
    })
}
