//! Post-vehicle-tick cargo age countdown and saturating packet ages.
use super::{RoadPhases, State, WorldTickError, number, set};
use crate::runtime::RuntimeError;

impl RoadPhases<'_> {
    pub(super) fn age_cargo(&mut self, state: &mut State<'_>) -> Result<(), WorldTickError> {
        let period = self.cache.cargo_age_period;
        if period == 0 {
            return Ok(());
        }
        let counter = number(state, self.id, false, "cargo_age_counter")?
            .min(i64::from(period))
            .wrapping_sub(1)
            & 65535;
        if counter == 0 {
            let cargo = self
                .transport
                .cargo
                .vehicles
                .get_mut(&self.id.raw())
                .ok_or(RuntimeError::Invalid("cargo vehicle age list"))?;
            for packet in &cargo.packets {
                let age = state.number(b"CAPA", *packet, "periods_in_transit")?;
                if age != i64::from(u16::MAX) {
                    state.set_number(
                        b"CAPA",
                        *packet,
                        "periods_in_transit",
                        age.wrapping_add(1),
                    )?;
                    cargo.periods = cargo
                        .periods
                        .wrapping_add(state.unsigned(b"CAPA", *packet, "count")?);
                }
            }
        }
        set(
            state,
            self.id,
            false,
            "cargo_age_counter",
            if counter == 0 {
                i64::from(period)
            } else {
                counter
            },
        )
    }
}
