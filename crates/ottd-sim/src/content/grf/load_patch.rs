use super::load_context::Environment;

#[derive(Debug, Clone, Copy)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent native settings consumed by InitializePatchFlags"
)]
pub(super) struct PatchSettings {
    pub never_expire_airports: bool,
    pub max_bridge_length: u16,
    pub never_expire_vehicles: bool,
    pub station_noise_level: bool,
    pub gradual_loading: bool,
    pub train_signal_side: u8,
    pub build_on_slopes: bool,
    pub wagon_speed_limits: bool,
    pub allow_town_roads: bool,
    pub generating_world: bool,
    pub improved_load: bool,
    pub dynamic_engines: bool,
    pub inflation: bool,
}

impl Environment {
    pub(super) fn patch_flags(&self, bit: &mut u32) -> u32 {
        let index = *bit / 32;
        *bit %= 32;
        let p = self.settings.patch;
        match index {
            0 => {
                (u32::from(p.never_expire_airports) << 12)
                    | (1 << 13)
                    | (1 << 14)
                    | (u32::from(p.max_bridge_length > 16) << 15)
                    | (1 << 18)
                    | (1 << 19)
                    | (u32::from(p.never_expire_vehicles) << 22)
                    | (1 << 27)
                    | (1 << 29)
                    | (1 << 30)
            }
            1 => {
                (u32::from(p.station_noise_level) << 7)
                    | (1 << 8)
                    | (1 << 9)
                    | (u32::from(p.gradual_loading) << 12)
                    | (1 << 18)
                    | (1 << 19)
                    | (1 << 20)
                    | (1 << 22)
                    | (1 << 23)
                    | (1 << 24)
                    | (1 << 25)
                    | (1 << 26)
                    | (u32::from(p.train_signal_side == 1) << 27)
                    | (u32::from(!self.settings.disable_elrails) << 28)
            }
            2 => {
                (1 << 1)
                    | (1 << 3)
                    | (1 << 10)
                    | (u32::from(p.build_on_slopes) << 13)
                    | (1 << 14)
                    | (1 << 15)
                    | (1 << 18)
                    | (1 << 19)
                    | (1 << 20)
                    | (u32::from(p.build_on_slopes) << 21)
                    | (1 << 22)
                    | (1 << 23)
                    | (u32::from(self.settings.freight_trains > 1) << 24)
                    | (1 << 25)
                    | (1 << 26)
                    | (1 << 27)
                    | (1 << 28)
                    | (u32::from(p.wagon_speed_limits) << 29)
                    | (1 << 30)
            }
            3 => {
                (1 << 1)
                    | (u32::from(!p.allow_town_roads && !p.generating_world) << 2)
                    | (1 << 3)
                    | (1 << 5)
                    | (1 << 6)
                    | (1 << 7)
                    | (u32::from(p.improved_load) << 8)
                    | (1 << 11)
                    | (1 << 12)
                    | (1 << 13)
                    | (1 << 14)
                    | (1 << 15)
                    | (1 << 16)
                    | (1 << 17)
                    | (1 << 18)
                    | (1 << 20)
                    | (1 << 22)
                    | (1 << 23)
                    | (u32::from(p.dynamic_engines) << 24)
                    | (1 << 30)
                    | (1 << 31)
            }
            4 => 1 | (u32::from(p.inflation) << 1) | (1 << 2),
            _ => 0,
        }
    }
    pub(super) fn patch_variable(&self, variable: u8) -> u32 {
        const SLOPES: u32 = 4896 + 192 + 240 + 65 + 8 + 12;
        const TWO_COLOUR: u32 = SLOPES + 74 + 90 + 55 + 48;
        match variable {
            0x0b => u32::from_ne_bytes(
                self.settings
                    .starting_year
                    .max(1920)
                    .saturating_sub(1920)
                    .to_ne_bytes(),
            ),
            0x0e => u32::from(self.settings.freight_trains),
            0x10 => match self.settings.plane_speed {
                1 => 4,
                2 | 3 => 2,
                _ => 1,
            },
            0x11 => TWO_COLOUR,
            0x13 => {
                let x = self.settings.map.width().ilog2().saturating_sub(6);
                let y = self.settings.map.height().ilog2().saturating_sub(6);
                let bits = match x.cmp(&y) {
                    std::cmp::Ordering::Equal => 1,
                    std::cmp::Ordering::Less => 2,
                    std::cmp::Ordering::Greater => 0,
                };
                (bits << 24)
                    | (x.min(y) << 20)
                    | (x.max(y) << 16)
                    | (x << 12)
                    | (y << 8)
                    | x.saturating_add(y)
            }
            0x14 => u32::from(self.settings.height_limit),
            0x15 => SLOPES,
            0x16 => TWO_COLOUR + 256,
            0x17 => self.settings.generation_seed,
            _ => 0,
        }
    }
}
