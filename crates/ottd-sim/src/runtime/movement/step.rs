use super::access::{number, random, set};
use super::{RoadPhases, SavedVehicleView, State, WorldTickError, unsupported};

// OpenTTD table/roadveh_movement.h: _roadveh_drive_data_16; terminal entry is NE tile transition.
const NE_RIGHT: [(i64, i64); 16] = [
    (15, 9),
    (14, 9),
    (13, 9),
    (12, 9),
    (11, 9),
    (10, 9),
    (9, 9),
    (8, 9),
    (7, 9),
    (6, 9),
    (5, 9),
    (4, 9),
    (3, 9),
    (2, 9),
    (1, 9),
    (0, 9),
];

impl RoadPhases<'_> {
    pub(super) fn tick(&mut self, state: &mut State<'_>) -> Result<(), WorldTickError> {
        let id = self.id;
        for name in ["tick_counter", "running_ticks"] {
            set(
                state,
                id,
                false,
                name,
                number(state, id, false, name)?.wrapping_add(1) & 255,
            )?;
        }
        set(
            state,
            id,
            false,
            "current_order_time",
            number(state, id, false, "current_order_time")?.wrapping_add(1) & 0xffff_ffff,
        )?;
        self.validate_position(state)?;
        let sub = number(state, id, false, "subspeed")?.wrapping_add(256);
        set(state, id, false, "subspeed", sub & 255)?;
        let speed = number(state, id, false, "cur_speed")?;
        let max = i64::from(self.cache.max_track_speed);
        let limit = if speed > max {
            speed.saturating_sub(speed / 10).saturating_sub(1).max(max)
        } else {
            max
        };
        let speed = speed.wrapping_add(sub >> 8).clamp(0, limit);
        set(state, id, false, "cur_speed", speed)?;
        let mut progress =
            (speed.wrapping_mul(3) / 4).wrapping_add(number(state, id, false, "progress")?);
        set(state, id, false, "progress", 0)?;
        while progress >= 192 {
            progress = progress.wrapping_sub(192);
            let frame = number(state, id, true, "frame")?;
            if frame == 15 {
                let tile = number(state, id, false, "tile")?
                    .checked_sub(1)
                    .ok_or_else(|| unsupported("road", "map edge"))?;
                self.normal_tile(state, tile)?;
                random(state)?;
                set(state, id, false, "tile", tile)?;
                set(state, id, true, "frame", 0)?;
            } else {
                set(state, id, true, "frame", frame.wrapping_add(1))?;
            }
            let (x, y) = self.position(state)?;
            set(state, id, false, "x_pos", x)?;
            set(state, id, false, "y_pos", y)?;
            let vehicle = SavedVehicleView::candidate(state.view(), id)?;
            self.spatial.update(vehicle, state.width)?;
            self.validate_position(state)?;
        }
        set(state, id, false, "progress", progress)?;
        self.cache.last_speed = u16::try_from(speed).map_err(|_| unsupported("road", "speed"))?;
        if self.cache.cargo_age_period != 0 {
            let counter = (number(state, id, false, "cargo_age_counter")?
                .min(i64::from(self.cache.cargo_age_period))
                .wrapping_sub(1))
                & 65535;
            set(
                state,
                id,
                false,
                "cargo_age_counter",
                if counter == 0 {
                    i64::from(self.cache.cargo_age_period)
                } else {
                    counter
                },
            )?;
        }
        set(
            state,
            id,
            false,
            "motion_counter",
            number(state, id, false, "motion_counter")?.wrapping_add(speed) & 0xffff_ffff,
        )?;
        Ok(())
    }
    fn normal_tile(&self, state: &State<'_>, tile: i64) -> Result<(), WorldTickError> {
        let tile = u32::try_from(tile).map_err(|_| unsupported("road", "tile"))?;
        let raw = state.view().tile(tile)?;
        if raw.tile_type() >> 4 != 2
            || raw.m5() != 10
            || raw.m4() & 63 != 0
            || raw.m3() & 15 != 0
            || raw.tile_type() & 0x0c != 0
        {
            return Err(unsupported("road", "straight normal vanilla road required"));
        }
        for offset in [1, state.width, state.width.wrapping_add(1)] {
            if state.view().tile(tile.wrapping_add(offset))?.height() != raw.height() {
                return Err(unsupported("road", "flat road required"));
            }
        }
        if number(state, self.id, false, "z_pos")? != i64::from(raw.height()).wrapping_mul(8) {
            return Err(unsupported("road", "road height transition"));
        }
        self.spatial
            .ensure_unoccupied(i64::from(tile), self.id, state.width)?;
        Ok(())
    }
    fn validate_position(&self, state: &State<'_>) -> Result<(), WorldTickError> {
        let tile = number(state, self.id, false, "tile")?;
        self.normal_tile(state, tile)?;
        let (x, y) = self.position(state)?;
        if number(state, self.id, false, "x_pos")? != x
            || number(state, self.id, false, "y_pos")? != y
        {
            return Err(unsupported("road", "NE movement-table position"));
        }
        Ok(())
    }
    fn position(&self, state: &State<'_>) -> Result<(i64, i64), WorldTickError> {
        let tile = number(state, self.id, false, "tile")?;
        let frame = usize::try_from(number(state, self.id, true, "frame")?)
            .map_err(|_| unsupported("road", "frame"))?;
        let &(x, y) = NE_RIGHT
            .get(frame)
            .ok_or_else(|| unsupported("road", "frame"))?;
        let width = i64::from(state.width);
        Ok((
            (tile & width.wrapping_sub(1))
                .wrapping_mul(16)
                .wrapping_add(x),
            (tile >> width.trailing_zeros())
                .wrapping_mul(16)
                .wrapping_add(y),
        ))
    }
}
