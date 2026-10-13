//! Original house properties consumed by construction and cargo production.
use serde::Serialize;

/// Pinned vanilla house tile specification, including zero-population footprint parts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HouseSpec {
    /// Population added when this tile completes construction.
    pub population: u8,
    /// Original per-tile mail production rate.
    pub mail_generation: u8,
    /// Native size, animation, church and stadium flag bits.
    pub building_flags: u8,
}
impl HouseSpec {
    /// Complete original catalog in native house-ID order.
    pub const fn vanilla() -> &'static [Self; 110] {
        &super::house_data::HOUSES
    }
    pub(crate) const fn is_north(self) -> bool {
        self.building_flags & 0x1d != 0
    }
    pub(crate) const fn size(self) -> (u32, u32) {
        (
            if self.building_flags & 0x14 != 0 {
                2
            } else {
                1
            },
            if self.building_flags & 0x18 != 0 {
                2
            } else {
                1
            },
        )
    }
}
