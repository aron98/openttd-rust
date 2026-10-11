#![expect(
    clippy::redundant_pub_crate,
    reason = "producer handoffs are shared internally across candidate phases"
)]
//! Neutral cargo producer identity and saved source area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CargoSource {
    Town(u16),
    Industry(u16),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TileArea {
    pub tile: u32,
    pub width: u16,
    pub height: u16,
}
#[derive(Debug, Clone, Copy)]
pub(crate) struct Production {
    pub source: CargoSource,
    pub area: TileArea,
    pub cargo: u8,
    pub amount: u32,
}
