use serde::Serialize;
/// Initialized gameplay cargo properties; invalid native slots are retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CargoSpec {
    /// Native `label` property.
    pub label: u32,
    /// Native `bitnum` property.
    pub bitnum: u8,
    /// Native `legend_colour` property.
    pub legend_colour: u8,
    /// Native `rating_colour` property.
    pub rating_colour: u8,
    /// Native `weight` property.
    pub weight: u8,
    /// Native `multiplier` property.
    pub multiplier: u16,
    /// Native `classes` property.
    pub classes: u16,
    /// Native `initial_payment` property.
    pub initial_payment: i32,
    /// Native `transit_periods` property.
    pub transit_periods: [u8; 2],
    /// Native `is_freight` property.
    pub is_freight: bool,
    /// Native `town_acceptance_effect` property.
    pub town_acceptance_effect: u8,
    /// Native `town_production_effect` property.
    pub town_production_effect: u8,
    /// Native `town_production_multiplier` property.
    pub town_production_multiplier: u16,
    /// Native `callback_mask` property.
    pub callback_mask: u8,
    /// Native `current_payment` property.
    pub current_payment: i64,
}
impl CargoSpec {
    pub(super) const EMPTY: Self = Self {
        label: 0,
        bitnum: 255,
        legend_colour: 0,
        rating_colour: 0,
        weight: 0,
        multiplier: 256,
        classes: 0,
        initial_payment: 0,
        transit_periods: [0, 0],
        is_freight: false,
        town_acceptance_effect: 0,
        town_production_effect: 0,
        town_production_multiplier: 256,
        callback_mask: 0,
        current_payment: 0,
    };
}
