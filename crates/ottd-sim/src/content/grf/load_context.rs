//! Native loading context from OpenTTD15.3 `newgrf.cpp` and `newgrf_actd.cpp`.
//! Source pin14ec60f248547d4d062a1160f0fc26d742319888; GPL-2.0-only.
use crate::content::Climate;
use ottd_core::{
    CalendarDate, ClockSnapshot, DateFraction, EconomyDate, InvalidDate, MapDimensions, TickCounter,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct EnvironmentSettings {
    pub display_options: u8,
    pub timekeeping_units: ottd_core::TimekeepingUnits,
    pub patch: super::load_patch::PatchSettings,
    pub game_mode: u8,
    pub starting_year: i32,
    pub climate: Climate,
    pub right_hand_traffic: bool,
    pub disable_elrails: bool,
    pub map: MapDimensions,
    pub height_limit: u8,
    pub snowline: u8,
    pub generation_seed: u32,
    pub freight_trains: u8,
    pub plane_speed: u8,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct EnvironmentInput {
    pub saved: ClockSnapshot,
    pub settings: EnvironmentSettings,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum GameMode {
    Menu,
    Normal,
    Editor,
    Bootstrap,
}
impl GameMode {
    pub(super) const fn raw(self) -> u32 {
        match self {
            Self::Menu => 0,
            Self::Normal => 1,
            Self::Editor => 2,
            Self::Bootstrap => 3,
        }
    }
}
impl TryFrom<u8> for GameMode {
    type Error = ContextError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Menu),
            1 => Ok(Self::Normal),
            2 => Ok(Self::Editor),
            3 => Ok(Self::Bootstrap),
            _ => Err(ContextError::Mode(value)),
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub(super) enum ContextError {
    #[error(transparent)]
    Calendar(#[from] InvalidDate),
    #[error("invalid native game mode {0}")]
    Mode(u8),
    #[error("context admission rejects starting year {0}")]
    StartingYear(i32),
    #[error("context admission rejects economy date {date} for {units:?}")]
    EconomyDate {
        date: i32,
        units: ottd_core::TimekeepingUnits,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FileGlobals {
    pub pitch: i32,
    pub width: u8,
}
impl Default for FileGlobals {
    fn default() -> Self {
        Self {
            pitch: 0,
            width: 29,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct Environment {
    pub saved_display: u8,
    pub current_display: u8,
    pub mode: GameMode,
    pub settings: EnvironmentSettings,
    pub saved: ClockSnapshot,
    pub current: ClockSnapshot,
    pub rail_costs: [u16; 4],
    pub misc: u8,
}

pub(super) type EnvironmentReport = (Environment, Vec<Option<FileGlobals>>);
impl Environment {
    pub(super) fn new(
        saved: ClockSnapshot,
        settings: EnvironmentSettings,
        networking: bool,
    ) -> Result<Self, ContextError> {
        let start = CalendarDate::from_ymd(settings.starting_year, 0, 1)
            .map_err(|_| ContextError::StartingYear(settings.starting_year))?;
        match settings.timekeeping_units {
            ottd_core::TimekeepingUnits::Calendar => CalendarDate::from_raw(saved.economy_date.0),
            ottd_core::TimekeepingUnits::Wallclock => {
                CalendarDate::from_ymd(saved.economy_date.0.div_euclid(360), 0, 1)
            }
        }
        .map_err(|_| ContextError::EconomyDate {
            date: saved.economy_date.0,
            units: settings.timekeeping_units,
        })?;
        let mut current = saved;
        if networking {
            current.date = start;
            current.date_fract = DateFraction(0);
            current.economy_date = EconomyDate(match settings.timekeeping_units {
                ottd_core::TimekeepingUnits::Calendar => start.raw(),
                ottd_core::TimekeepingUnits::Wallclock => {
                    settings.starting_year.checked_mul(360).ok_or(InvalidDate)?
                }
            });
            current.economy_date_fract = DateFraction(0);
            current.tick_counter = TickCounter(0);
        }
        Ok(Self {
            saved_display: settings.display_options,
            current_display: if networking {
                0
            } else {
                settings.display_options
            },
            mode: GameMode::try_from(settings.game_mode)?,
            settings,
            saved,
            current,
            rail_costs: [8, 12, 16, 24],
            misc: 0,
        })
    }
    pub(super) const fn restore_clock(&mut self) {
        self.current = self.saved;
        self.current_display = self.saved_display;
    }
    pub(super) fn target(
        &mut self,
        target: u8,
        value: u32,
        is_static: bool,
        file: &mut FileGlobals,
    ) {
        let [low, middle, high, _] = value.to_le_bytes();
        match target {
            0x8e => file.pitch = i32::from_ne_bytes(value.to_ne_bytes()),
            0x8f => {
                self.rail_costs = if self.settings.disable_elrails {
                    [
                        u16::from(low),
                        u16::from(low),
                        u16::from(middle),
                        u16::from(high),
                    ]
                } else {
                    [
                        u16::from(low),
                        u16::from(middle),
                        u16::from(high),
                        u16::from(high),
                    ]
                }
            }
            0x9e => {
                file.width = if low & 8 != 0 { 32 } else { 29 };
                let global = low & !8;
                self.misc = if is_static {
                    (self.misc & !64) | (global & 64)
                } else {
                    global
                };
            }
            _ => (),
        }
    }
}
