//! Source port of native `newgrf_act0_globalvar.cpp` global-variable lookup.
//! OpenTTD15.3 pin14ec60f248547d4d062a1160f0fc26d742319888; GPL-2.0-only.
use super::{
    Palette,
    load_context::{Environment, FileGlobals},
};
use crate::content::Climate;
use ottd_core::{CalendarDate, InvalidDate};

impl Environment {
    pub(super) fn global(
        &self,
        number: u8,
        file: FileGlobals,
        version: u8,
        palette: Palette,
    ) -> Result<Option<u32>, InvalidDate> {
        let date = self.current.date;
        let (year, month, day) = date.ymd();
        let value = match number {
            0 => u32::try_from(
                date.raw()
                    .saturating_sub(CalendarDate::from_ymd(1920, 0, 1)?.raw())
                    .max(0),
            )
            .map_err(|_| InvalidDate)?,
            1 => u32::try_from(year.clamp(1920, 2090).saturating_sub(1920))
                .map_err(|_| InvalidDate)?,
            2 => {
                let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
                let ordinal = u32::try_from(
                    date.raw()
                        .saturating_sub(CalendarDate::from_ymd(year, 0, 1)?.raw()),
                )
                .map_err(|_| InvalidDate)?;
                u32::from(month)
                    | (u32::from(day.saturating_sub(1)) << 8)
                    | (u32::from(leap) << 15)
                    | (ordinal << 16)
            }
            3 => match self.settings.climate {
                Climate::Temperate => 0,
                Climate::Arctic => 1,
                Climate::Tropic => 2,
                Climate::Toyland => 3,
            },
            6 => u32::from(self.settings.right_hand_traffic) << 4,
            9 => u32::from(self.current.date_fract.0).wrapping_mul(885),
            0x0a => {
                let [a, b, ..] = self.current.tick_counter.0.to_le_bytes();
                u32::from(u16::from_le_bytes([a, b]))
            }
            0x0b => (2 << 24) | (6 << 20) | (1 << 16) | 0x0566,
            0x0d => match palette {
                Palette::Dos => 0,
                Palette::Windows => 1,
            },
            0x0e => u32::from_ne_bytes(file.pitch.to_ne_bytes()),
            0x0f => {
                let [rail, electric, mono, maglev] = self.rail_costs;
                u32::from(rail & 255)
                    | (u32::from(
                        if self.settings.disable_elrails {
                            mono
                        } else {
                            electric
                        } & 255,
                    ) << 8)
                    | (u32::from(maglev & 255) << 16)
            }
            0x11 => 0,
            0x12 => self.mode.raw(),
            0x1a => u32::MAX,
            0x1b => 0x3f,
            0x1d => 1,
            0x1e => u32::from(self.misc | if file.width == 32 { 8 } else { 0 }),
            0x20 => {
                if self.settings.climate == Climate::Arctic
                    && self.settings.snowline <= self.settings.height_limit
                {
                    u32::from(self.settings.snowline)
                        .saturating_mul(if version >= 8 { 1 } else { 8 })
                        .min(254)
                } else {
                    255
                }
            }
            0x21 => (31 << 24) | (3 << 20) | (1 << 19) | 0x6d64,
            0x22 => 3,
            0x23 => u32::from_ne_bytes(date.raw().to_ne_bytes()),
            0x24 => u32::from_ne_bytes(year.to_ne_bytes()),
            _ => return Ok(None),
        };
        Ok(Some(value))
    }
}
