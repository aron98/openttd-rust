use super::RuntimeError;
use serde::Serialize;
use std::collections::BTreeSet;

/// Inclusive tile coordinates of a nonempty station area.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct StationArea {
    /// Minimum tile X coordinate.
    pub left: u32,
    /// Minimum tile Y coordinate.
    pub top: u32,
    /// Maximum tile X coordinate.
    pub right: u32,
    /// Maximum tile Y coordinate.
    pub bottom: u32,
}
impl StationArea {
    pub(super) fn tile(tile: u32, width: u32) -> Result<Self, RuntimeError> {
        let width = std::num::NonZeroU32::new(width).ok_or(RuntimeError::Invalid("map width"))?;
        let (x, y) = (tile % width, tile / width);
        Ok(Self {
            left: x,
            top: y,
            right: x,
            bottom: y,
        })
    }
    pub(super) fn include(&mut self, other: Self) {
        self.left = self.left.min(other.left);
        self.top = self.top.min(other.top);
        self.right = self.right.max(other.right);
        self.bottom = self.bottom.max(other.bottom);
    }
    pub(crate) const fn dimensions(self) -> (u32, u32) {
        (
            self.right.saturating_sub(self.left).saturating_add(1),
            self.bottom.saturating_sub(self.top).saturating_add(1),
        )
    }
}

/// Restored road-only station geometry; saved facility chains remain authoritative.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct RoadStationGeometry {
    /// Bounding rectangle of station facilities.
    pub station: Option<StationArea>,
    /// Bounding rectangle of bus stops.
    pub bus: Option<StationArea>,
    /// Bounding rectangle of truck stops.
    pub truck: Option<StationArea>,
    /// Union of covered tile indices in native map iteration order.
    pub catchment: BTreeSet<u32>,
}
impl RoadStationGeometry {
    pub(crate) fn add(
        &mut self,
        tile: u32,
        bus: bool,
        dimensions: (u32, u32),
        radius: u32,
    ) -> Result<(), RuntimeError> {
        let (width, height) = dimensions;
        let area = StationArea::tile(tile, width)?;
        if area.left >= width || area.top >= height {
            return Err(RuntimeError::Invalid("station tile bounds"));
        }
        for target in [
            &mut self.station,
            if bus { &mut self.bus } else { &mut self.truck },
        ] {
            match target {
                Some(value) => value.include(area),
                None => *target = Some(area),
            }
        }
        let max_x = width
            .checked_sub(1)
            .ok_or(RuntimeError::Invalid("map width"))?;
        let max_y = height
            .checked_sub(1)
            .ok_or(RuntimeError::Invalid("map height"))?;
        for y in area.top.saturating_sub(radius)..=area.bottom.saturating_add(radius).min(max_y) {
            for x in area.left.saturating_sub(radius)..=area.right.saturating_add(radius).min(max_x)
            {
                let index = y
                    .checked_mul(width)
                    .and_then(|row| row.checked_add(x))
                    .ok_or(RuntimeError::Invalid("station catchment tile"))?;
                self.catchment.insert(index);
            }
        }
        Ok(())
    }
}
