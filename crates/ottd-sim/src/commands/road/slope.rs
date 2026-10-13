use crate::commands::CommandCost;
use ottd_core::terrain::{Corner, Foundation, Slope};

const INVALID_LEVELED: [u8; 15] = [0, 12, 9, 8, 3, 0, 1, 0, 6, 4, 0, 0, 2, 0, 0];
const INVALID_STRAIGHT: [u8; 15] = [0, 0, 0, 5, 0, 15, 10, 15, 0, 10, 15, 15, 5, 15, 15];

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
enum InputError {
    #[error("road helper requires a natural ground slope")]
    Slope,
    #[error("requested road bits exceed four bits")]
    RequestedBits,
    #[error("existing road bits exceed four bits")]
    ExistingBits,
    #[error("other road bits exceed four bits")]
    OtherBits,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Input {
    slope: u8,
    invalid_leveled: u8,
    invalid_straight: u8,
    requested: u8,
    existing: u8,
    other: u8,
    enabled: bool,
    price: i64,
}
impl Input {
    fn new(
        raw: u8,
        requested: u8,
        existing: u8,
        other: u8,
        enabled: bool,
        price: i64,
    ) -> Result<Self, InputError> {
        if raw == 15 || raw >= 32 {
            return Err(InputError::Slope);
        }
        let slope = Slope::new(raw).map_err(|_| InputError::Slope)?;
        let mut reduced = raw;
        if raw & 16 != 0 {
            for corner in [Corner::West, Corner::South, Corner::East, Corner::North] {
                if slope.corner_z(corner).map_err(|_| InputError::Slope)? == 2 {
                    reduced = 1 << (corner as u8);
                }
            }
        }
        for (bits, error) in [
            (requested, InputError::RequestedBits),
            (existing, InputError::ExistingBits),
            (other, InputError::OtherBits),
        ] {
            if bits > 15 {
                return Err(error);
            }
        }
        Ok(Self {
            slope: reduced,
            invalid_leveled: *INVALID_LEVELED
                .get(usize::from(reduced))
                .ok_or(InputError::Slope)?,
            invalid_straight: *INVALID_STRAIGHT
                .get(usize::from(reduced))
                .ok_or(InputError::Slope)?,
            requested,
            existing,
            other,
            enabled,
            price,
        })
    }

    const fn foundation(self, bits: u8) -> Foundation {
        if self.slope == 0 || bits == 0 {
            Foundation::None
        } else if self.invalid_leveled & bits == 0 {
            Foundation::Leveled
        } else if !self.slope.is_power_of_two() && self.invalid_straight & bits == 0 {
            Foundation::None
        } else if bits == 10 {
            Foundation::InclinedX
        } else {
            Foundation::InclinedY
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Output {
    pieces: u8,
    cost: CommandCost,
}

fn check(input: Input) -> Output {
    let mut pieces = input.requested & !input.existing;
    if pieces == 0 {
        return Output {
            pieces,
            cost: CommandCost::failure("CMD_ERROR"),
        };
    }
    if input.slope == 0 {
        return Output {
            pieces,
            cost: CommandCost::success(0, 255),
        };
    }
    if input.enabled && input.invalid_leveled & (input.other | input.existing | pieces) == 0 {
        let cost = if input.other | input.existing == 0 {
            CommandCost::success(input.price, 0)
        } else {
            CommandCost::success(0, 255)
        };
        return Output { pieces, cost };
    }
    pieces |= ((pieces << 2) | (pieces >> 2)) & 15;
    let combined = input.existing | pieces;
    let mut cost = CommandCost::failure("CMD_ERROR");
    if matches!(combined, 5 | 10)
        && (input.other == combined || input.other == 0)
        && input.invalid_straight & (input.other | combined) == 0
    {
        if input.slope.is_power_of_two() {
            if input.enabled {
                cost = if input.other | input.existing == 0 {
                    CommandCost::success(input.price, 0)
                } else {
                    CommandCost::success(0, 255)
                };
            }
        } else {
            cost = if input.existing.is_power_of_two()
                && input.foundation(input.existing) == Foundation::None
            {
                CommandCost::success(input.price, 0)
            } else {
                CommandCost::success(0, 255)
            };
        }
    }
    Output { pieces, cost }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod native;
