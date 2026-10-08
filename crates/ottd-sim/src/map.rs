use serde::{Deserialize, Serialize};

/// Raw upstream tile fields owned by the mutable subsystem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tile {
    /// Type nibble and tile flags.
    #[serde(rename = "type")]
    pub tile_type: u8,
    /// Height above sea level.
    pub height: u8,
    /// Raw m1 field.
    pub m1: u8,
    /// Raw m2 field.
    pub m2: u16,
    /// Raw m3 field.
    pub m3: u8,
    /// Raw m4 field.
    pub m4: u8,
    /// Clear ground, counter and density.
    pub m5: u8,
    /// Raw m6 field.
    pub m6: u8,
    /// Raw m7 field.
    pub m7: u8,
    /// Raw m8 field.
    pub m8: u16,
}

/// Unvalidated map input; [`crate::Landscape::new`] validates it before use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Map {
    /// Width in tiles.
    pub width: u32,
    /// Height in tiles.
    pub height: u32,
    /// Raw tiles in upstream linear order.
    pub tiles: Vec<Tile>,
}
