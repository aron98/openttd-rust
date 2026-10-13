//! Native terrain geometry, ported from OpenTTD 15.3 `landscape.cpp` and `slope_func.h`.

/// An undefined native encoding, coordinate, or incompatible geometry operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum GeometryError {
    /// Undefined slope bits.
    #[error("invalid native slope")]
    Slope,
    /// Undefined corner or diagonal edge.
    #[error("invalid corner or edge")]
    Direction,
    /// Undefined foundation or a foundation without the required highest corner.
    #[error("invalid foundation or incompatible slope")]
    Foundation,
    /// Pixel outside one 16-by-16 tile.
    #[error("tile pixel must be in 0..16")]
    Pixel,
    /// Discontinuous slopes have edge-dependent corner heights.
    #[error("halftile slope has no unique corner height")]
    Discontinuous,
    /// Adjacent corners differ by more than one height unit.
    #[error("invalid tile corner heights")]
    Heights,
    /// Tile index outside the map.
    #[error("tile outside map")]
    Tile,
}

/// Native tile corner order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Corner {
    /// Western corner.
    West = 0,
    /// Southern corner.
    South = 1,
    /// Eastern corner.
    East = 2,
    /// Northern corner.
    North = 3,
}
impl TryFrom<u8> for Corner {
    type Error = GeometryError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::West),
            1 => Ok(Self::South),
            2 => Ok(Self::East),
            3 => Ok(Self::North),
            _ => Err(GeometryError::Direction),
        }
    }
}
impl Corner {
    const fn bit(self) -> u8 {
        1 << (self as u8)
    }
    const fn steep(self) -> u8 {
        16 | (15 ^ (1 << ((self as u8) ^ 2)))
    }
}

/// Native diagonal edge order; endpoint order is near then far from the camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Edge {
    /// Eastern and northern endpoints.
    NorthEast = 0,
    /// Southern and eastern endpoints.
    SouthEast = 1,
    /// Southern and western endpoints.
    SouthWest = 2,
    /// Western and northern endpoints.
    NorthWest = 3,
}
impl TryFrom<u8> for Edge {
    type Error = GeometryError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::NorthEast),
            1 => Ok(Self::SouthEast),
            2 => Ok(Self::SouthWest),
            3 => Ok(Self::NorthWest),
            _ => Err(GeometryError::Direction),
        }
    }
}

/// Native foundation operation; selection for a track or road belongs to its tile procedure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Foundation {
    /// Retain the original slope.
    None = 0,
    /// Flatten at the upper level.
    Leveled = 1,
    /// Incline along the X axis.
    InclinedX = 2,
    /// Incline along the Y axis.
    InclinedY = 3,
    /// Raise the lower half of a steep tile.
    SteepLower = 4,
    /// Raise the lower half and level the upper half.
    SteepBoth = 5,
    /// Level the western half.
    HalfWest = 6,
    /// Level the southern half.
    HalfSouth = 7,
    /// Level the eastern half.
    HalfEast = 8,
    /// Level the northern half.
    HalfNorth = 9,
    /// Western rail foundation.
    RailWest = 10,
    /// Southern rail foundation.
    RailSouth = 11,
    /// Eastern rail foundation.
    RailEast = 12,
    /// Northern rail foundation.
    RailNorth = 13,
}
impl TryFrom<u8> for Foundation {
    type Error = GeometryError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Leveled),
            2 => Ok(Self::InclinedX),
            3 => Ok(Self::InclinedY),
            4 => Ok(Self::SteepLower),
            5 => Ok(Self::SteepBoth),
            6 => Ok(Self::HalfWest),
            7 => Ok(Self::HalfSouth),
            8 => Ok(Self::HalfEast),
            9 => Ok(Self::HalfNorth),
            10 => Ok(Self::RailWest),
            11 => Ok(Self::RailSouth),
            12 => Ok(Self::RailEast),
            13 => Ok(Self::RailNorth),
            _ => Err(GeometryError::Foundation),
        }
    }
}

/// A checked pixel coordinate within one tile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TilePixel(u8);
impl TilePixel {
    /// Check the native local-coordinate domain.
    /// # Errors
    /// Rejects coordinates greater than 15.
    pub const fn new(value: u8) -> Result<Self, GeometryError> {
        if value < 16 {
            Ok(Self(value))
        } else {
            Err(GeometryError::Pixel)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum BaseSlope {
    Flat = 0,
    W = 1,
    S = 2,
    Sw = 3,
    E = 4,
    Ew = 5,
    Se = 6,
    Wse = 7,
    N = 8,
    Nw = 9,
    Ns = 10,
    Nws = 11,
    Ne = 12,
    Enw = 13,
    Sen = 14,
    Elevated = 15,
    SteepS = 23,
    SteepW = 27,
    SteepN = 29,
    SteepE = 30,
}

/// A defined native slope, optionally with a discontinuous halftile foundation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slope {
    base: BaseSlope,
    half: Option<Corner>,
}
impl Slope {
    /// Decode native bits without permitting undefined steep shapes or stray corner bits.
    /// # Errors
    /// Rejects undefined slope bytes.
    pub fn new(raw: u8) -> Result<Self, GeometryError> {
        let base = match raw & 31 {
            0 => BaseSlope::Flat,
            1 => BaseSlope::W,
            2 => BaseSlope::S,
            3 => BaseSlope::Sw,
            4 => BaseSlope::E,
            5 => BaseSlope::Ew,
            6 => BaseSlope::Se,
            7 => BaseSlope::Wse,
            8 => BaseSlope::N,
            9 => BaseSlope::Nw,
            10 => BaseSlope::Ns,
            11 => BaseSlope::Nws,
            12 => BaseSlope::Ne,
            13 => BaseSlope::Enw,
            14 => BaseSlope::Sen,
            15 => BaseSlope::Elevated,
            23 => BaseSlope::SteepS,
            27 => BaseSlope::SteepW,
            29 => BaseSlope::SteepN,
            30 => BaseSlope::SteepE,
            _ => return Err(GeometryError::Slope),
        };
        let half = if raw & 32 != 0 {
            Some(Corner::try_from(raw >> 6)?)
        } else if raw & 0xC0 == 0 {
            None
        } else {
            return Err(GeometryError::Slope);
        };
        Ok(Self { base, half })
    }
    /// Return the exact native byte representation.
    pub const fn raw(self) -> u8 {
        (self.base as u8)
            | match self.half {
                Some(corner) => 32 | ((corner as u8) << 6),
                None => 0,
            }
    }
    const fn steep(self) -> bool {
        self.base as u8 & 16 != 0
    }
    const fn highest(self) -> Option<Corner> {
        match self.base {
            BaseSlope::W | BaseSlope::SteepW => Some(Corner::West),
            BaseSlope::S | BaseSlope::SteepS => Some(Corner::South),
            BaseSlope::E | BaseSlope::SteepE => Some(Corner::East),
            BaseSlope::N | BaseSlope::SteepN => Some(Corner::North),
            BaseSlope::Flat
            | BaseSlope::Sw
            | BaseSlope::Ew
            | BaseSlope::Se
            | BaseSlope::Wse
            | BaseSlope::Nw
            | BaseSlope::Ns
            | BaseSlope::Nws
            | BaseSlope::Ne
            | BaseSlope::Enw
            | BaseSlope::Sen
            | BaseSlope::Elevated => None,
        }
    }
    const fn continuous_corner_z(self, corner: Corner) -> u8 {
        let raised = if self.base as u8 & corner.bit() != 0 {
            1_u8
        } else {
            0
        };
        raised.wrapping_add(if self.base as u8 == corner.steep() {
            1
        } else {
            0
        })
    }
    /// Corner height in terrain levels above the tile's base.
    /// # Errors
    /// Halftile slopes have no unique height at a corner; use edge heights.
    pub const fn corner_z(self, corner: Corner) -> Result<u8, GeometryError> {
        if self.half.is_some() {
            Err(GeometryError::Discontinuous)
        } else {
            Ok(self.continuous_corner_z(corner))
        }
    }
    /// Near/far edge endpoint offsets in pixels, including discontinuous foundations.
    pub fn edge_pixel_z(self, edge: Edge) -> (u8, u8) {
        let (near, far) = match edge {
            Edge::NorthEast => (Corner::East, Corner::North),
            Edge::SouthEast => (Corner::South, Corner::East),
            Edge::SouthWest => (Corner::South, Corner::West),
            Edge::NorthWest => (Corner::West, Corner::North),
        };
        (
            self.continuous_corner_z(near)
                .wrapping_add(u8::from(self.half == Some(far)))
                .wrapping_mul(8),
            self.continuous_corner_z(far)
                .wrapping_add(u8::from(self.half == Some(near)))
                .wrapping_mul(8),
        )
    }
    /// Native pixel height above the tile base, with exact integer rounding and seams.
    pub fn partial_pixel_z(self, x: TilePixel, y: TilePixel) -> u8 {
        let (x, y) = (x.0, y.0);
        let sum = x.wrapping_add(y);
        if self.half.is_some_and(|corner| match corner {
            Corner::West => x > y,
            Corner::South => sum >= 16,
            Corner::East => x <= y,
            Corner::North => sum < 16,
        }) {
            return if self.steep() { 16 } else { 8 };
        }
        let north = 16_u8.saturating_sub(sum) >> 1;
        let south = sum.wrapping_add(1).saturating_sub(16) >> 1;
        let west = x.saturating_sub(y) >> 1;
        let east = y.wrapping_add(1).saturating_sub(x) >> 1;
        match self.base {
            BaseSlope::Flat => 0,
            BaseSlope::N => north,
            BaseSlope::S => south,
            BaseSlope::W => west,
            BaseSlope::E => east,
            BaseSlope::Ne => 16_u8.wrapping_sub(x) >> 1,
            BaseSlope::Se => y.wrapping_add(1) >> 1,
            BaseSlope::Sw => x.wrapping_add(1) >> 1,
            BaseSlope::Nw => 16_u8.wrapping_sub(y) >> 1,
            BaseSlope::Enw => 8_u8.wrapping_sub(south),
            BaseSlope::Sen => 8_u8.wrapping_sub(west),
            BaseSlope::Wse => 8_u8.wrapping_sub(north),
            BaseSlope::Nws => 8_u8.wrapping_sub(east),
            BaseSlope::Ns => {
                if sum < 16 {
                    north
                } else {
                    south
                }
            }
            BaseSlope::Ew => {
                if x >= y {
                    west
                } else {
                    east
                }
            }
            BaseSlope::Elevated => 8,
            BaseSlope::SteepN => 32_u8.wrapping_sub(sum) >> 1,
            BaseSlope::SteepE => 17_u8.wrapping_add(y).wrapping_sub(x) >> 1,
            BaseSlope::SteepS => sum.wrapping_add(1) >> 1,
            BaseSlope::SteepW => 16_u8.wrapping_add(x).wrapping_sub(y) >> 1,
        }
    }
    /// Apply a native foundation, returning the upper slope and base rise in terrain levels.
    /// # Errors
    /// Rejects already-discontinuous slopes and operations requiring a missing highest corner.
    pub fn apply_foundation(self, foundation: Foundation) -> Result<(Self, u8), GeometryError> {
        if self.half.is_some() {
            return Err(GeometryError::Foundation);
        }
        let (raw, dz) = match foundation {
            Foundation::None => (self.raw(), 0),
            Foundation::Leveled => (0, 1_u8.wrapping_add(u8::from(self.steep()))),
            Foundation::HalfWest
            | Foundation::HalfSouth
            | Foundation::HalfEast
            | Foundation::HalfNorth => (
                self.raw() | 32 | ((foundation as u8).wrapping_sub(6) << 6),
                0,
            ),
            Foundation::RailWest
            | Foundation::RailSouth
            | Foundation::RailEast
            | Foundation::RailNorth => (15 ^ (1 << ((foundation as u8).wrapping_sub(10) ^ 2)), 0),
            Foundation::InclinedX
            | Foundation::InclinedY
            | Foundation::SteepLower
            | Foundation::SteepBoth => {
                let corner = self.highest().ok_or(GeometryError::Foundation)?;
                let raw = match foundation {
                    Foundation::InclinedX => {
                        if matches!(corner, Corner::West | Corner::South) {
                            3
                        } else {
                            12
                        }
                    }
                    Foundation::InclinedY => {
                        if matches!(corner, Corner::South | Corner::East) {
                            6
                        } else {
                            9
                        }
                    }
                    Foundation::SteepLower => corner.bit(),
                    Foundation::SteepBoth => corner.bit() | 32 | ((corner as u8) << 6),
                    Foundation::None
                    | Foundation::Leveled
                    | Foundation::HalfWest
                    | Foundation::HalfSouth
                    | Foundation::HalfEast
                    | Foundation::HalfNorth
                    | Foundation::RailWest
                    | Foundation::RailSouth
                    | Foundation::RailEast
                    | Foundation::RailNorth => return Err(GeometryError::Foundation),
                };
                (raw, u8::from(self.steep()))
            }
        };
        Ok((Self::new(raw)?, dz))
    }
}

/// Derive the native slope and minimum height from N, W, E, S corner heights.
/// # Errors
/// Rejects surfaces whose neighboring corner heights differ by more than one level.
pub fn slope_from_corners(
    [north, west, east, south]: [u8; 4],
) -> Result<(Slope, u8), GeometryError> {
    if [(north, west), (north, east), (south, west), (south, east)]
        .into_iter()
        .any(|(a, b)| a.abs_diff(b) > 1)
    {
        return Err(GeometryError::Heights);
    }
    let min = north.min(west).min(east).min(south);
    let max = north.max(west).max(east).max(south);
    let raw = (u8::from(north != min) << 3)
        | u8::from(west != min)
        | (u8::from(east != min) << 2)
        | (u8::from(south != min) << 1)
        | if max.wrapping_sub(min) == 2 { 16 } else { 0 };
    Ok((Slope::new(raw)?, min))
}
