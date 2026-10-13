use super::{AircraftSpec, RailSpec, RoadSpec, ShipSpec};
use serde::Serialize;

/// Native landscape selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[repr(u8)]
pub enum Climate {
    /// Temperate climate.
    Temperate = 0,
    /// Sub-arctic climate.
    Arctic = 1,
    /// Sub-tropical climate.
    Tropic = 2,
    /// Toyland climate.
    Toyland = 3,
}

/// Native engine cargo-label variant before climate resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CargoLabelSource {
    /// A fixed four-byte native cargo label.
    Fixed(u32),
    /// First available livestock or fruit label.
    LivestockFruit,
    /// First available grain, wheat or maize label.
    GrainWheatMaize,
    /// First available valuables, gold or diamonds label.
    ValuablesGoldDiamonds,
}

/// Static gameplay fields of native `EngineInfo`, after cargo finalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EngineInfo {
    /// Introduction date in days since year zero, before randomization.
    pub base_intro: i32,
    /// Vehicle lifetime in years.
    pub lifelength: i32,
    /// Engine availability lifetime in years; 255 denotes infinite.
    pub base_life: i32,
    /// Reliability decay speed.
    pub decay_speed: u8,
    /// Cargo units loaded per loading step.
    pub load_amount: u8,
    /// Native landscape bitmask.
    pub climates: u8,
    /// Default cargo slot; 255 denotes invalid.
    pub cargo_type: u8,
    /// Native label or mixed-label selection.
    pub cargo_label: CargoLabelSource,
    /// Refit cargo-slot mask.
    pub refit_mask: u64,
    /// Refit cost factor.
    pub refit_cost: u8,
    /// Native misc flags.
    pub misc_flags: u8,
    /// Callback mask.
    pub callback_mask: u16,
    /// Years early to retire.
    pub retire_early: i8,
    /// Extended engine flags.
    pub extra_flags: u8,
    /// Ticks between cargo aging.
    pub cargo_age_period: u16,
    /// Parent variant ID; 65535 denotes none.
    pub variant_id: u16,
}

/// Type-specific native engine properties.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum VehicleSpec {
    /// Train or wagon.
    Rail(RailSpec),
    /// Road vehicle.
    Road(RoadSpec),
    /// Ship.
    Ship(ShipSpec),
    /// Aircraft.
    Aircraft(AircraftSpec),
}

/// A finalized original engine; excludes mutable saved age/reliability state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct EngineSpec {
    /// Global native engine pool index.
    pub id: u16,
    /// Original engine index within its vehicle type.
    pub local_id: u16,
    /// Shared gameplay properties.
    pub info: EngineInfo,
    /// Type-specific gameplay properties.
    pub vehicle: VehicleSpec,
}
