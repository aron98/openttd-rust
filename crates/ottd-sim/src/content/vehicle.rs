use super::Price;
use serde::Serialize;
/// Native Rail vehicle properties, in original field units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RailSpec {
    /// Native `image_index` property.
    pub image_index: u8,
    /// Native `railveh_type` property.
    pub railveh_type: u8,
    /// Native `cost_factor` property.
    pub cost_factor: u8,
    /// Native `railtypes` property.
    pub railtypes: u64,
    /// Native `intended_railtypes` property.
    pub intended_railtypes: u64,
    /// Native `ai_passenger_only` property.
    pub ai_passenger_only: u8,
    /// Native `max_speed` property.
    pub max_speed: u16,
    /// Native `power` property.
    pub power: u16,
    /// Native `weight` property.
    pub weight: u16,
    /// Native `running_cost` property.
    pub running_cost: u8,
    /// Native `running_cost_class` property.
    pub running_cost_class: Option<Price>,
    /// Native `engclass` property.
    pub engclass: u8,
    /// Native `capacity` property.
    pub capacity: u8,
    /// Native `pow_wag_power` property.
    pub pow_wag_power: u16,
    /// Native `pow_wag_weight` property.
    pub pow_wag_weight: u8,
    /// Native `visual_effect` property.
    pub visual_effect: u8,
    /// Native `shorten_factor` property.
    pub shorten_factor: u8,
    /// Native `tractive_effort` property.
    pub tractive_effort: u8,
    /// Native `air_drag` property.
    pub air_drag: u8,
    /// Native `user_def_data` property.
    pub user_def_data: u8,
    /// Native `curve_speed_mod` property.
    pub curve_speed_mod: i16,
}
impl RailSpec {
    pub(super) const DEFAULT: Self = Self {
        image_index: 0,
        railveh_type: 0,
        cost_factor: 0,
        railtypes: 0,
        intended_railtypes: 0,
        ai_passenger_only: 0,
        max_speed: 0,
        power: 0,
        weight: 0,
        running_cost: 0,
        running_cost_class: None,
        engclass: 0,
        capacity: 0,
        pow_wag_power: 0,
        pow_wag_weight: 0,
        visual_effect: 255,
        shorten_factor: 0,
        tractive_effort: 76,
        air_drag: 0,
        user_def_data: 0,
        curve_speed_mod: 0,
    };
}
/// Native Road vehicle properties, in original field units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RoadSpec {
    /// Native `image_index` property.
    pub image_index: u8,
    /// Native `cost_factor` property.
    pub cost_factor: u8,
    /// Native `running_cost` property.
    pub running_cost: u8,
    /// Native `running_cost_class` property.
    pub running_cost_class: Option<Price>,
    /// Native `sfx` property.
    pub sfx: u16,
    /// Native `max_speed` property.
    pub max_speed: u16,
    /// Native `capacity` property.
    pub capacity: u8,
    /// Native `weight` property.
    pub weight: u8,
    /// Native `power` property.
    pub power: u8,
    /// Native `tractive_effort` property.
    pub tractive_effort: u8,
    /// Native `air_drag` property.
    pub air_drag: u8,
    /// Native `visual_effect` property.
    pub visual_effect: u8,
    /// Native `shorten_factor` property.
    pub shorten_factor: u8,
    /// Native `roadtype` property.
    pub roadtype: u8,
}
impl RoadSpec {
    pub(super) const DEFAULT: Self = Self {
        image_index: 0,
        cost_factor: 0,
        running_cost: 0,
        running_cost_class: None,
        sfx: 0,
        max_speed: 0,
        capacity: 0,
        weight: 0,
        power: 0,
        tractive_effort: 76,
        air_drag: 0,
        visual_effect: 255,
        shorten_factor: 0,
        roadtype: 0,
    };
}
/// Native Ship vehicle properties, in original field units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ShipSpec {
    /// Native `image_index` property.
    pub image_index: u8,
    /// Native `cost_factor` property.
    pub cost_factor: u8,
    /// Native `running_cost` property.
    pub running_cost: u8,
    /// Native `acceleration` property.
    pub acceleration: u8,
    /// Native `max_speed` property.
    pub max_speed: u16,
    /// Native `capacity` property.
    pub capacity: u16,
    /// Native `sfx` property.
    pub sfx: u16,
    /// Native `old_refittable` property.
    pub old_refittable: bool,
    /// Native `visual_effect` property.
    pub visual_effect: u8,
    /// Native `ocean_speed_frac` property.
    pub ocean_speed_frac: u8,
    /// Native `canal_speed_frac` property.
    pub canal_speed_frac: u8,
}
impl ShipSpec {
    pub(super) const DEFAULT: Self = Self {
        image_index: 0,
        cost_factor: 0,
        running_cost: 0,
        acceleration: 1,
        max_speed: 0,
        capacity: 0,
        sfx: 0,
        old_refittable: false,
        visual_effect: 255,
        ocean_speed_frac: 0,
        canal_speed_frac: 0,
    };
}
/// Native Aircraft vehicle properties, in original field units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AircraftSpec {
    /// Native `image_index` property.
    pub image_index: u8,
    /// Native `cost_factor` property.
    pub cost_factor: u8,
    /// Native `running_cost` property.
    pub running_cost: u8,
    /// Native `subtype` property.
    pub subtype: u8,
    /// Native `sfx` property.
    pub sfx: u16,
    /// Native `max_speed` property.
    pub max_speed: u16,
    /// Native `acceleration` property.
    pub acceleration: u8,
    /// Native `mail_capacity` property.
    pub mail_capacity: u8,
    /// Native `passenger_capacity` property.
    pub passenger_capacity: u16,
    /// Native `max_range` property.
    pub max_range: u16,
}
impl AircraftSpec {
    pub(super) const DEFAULT: Self = Self {
        image_index: 0,
        cost_factor: 0,
        running_cost: 0,
        subtype: 0,
        sfx: 0,
        max_speed: 0,
        acceleration: 0,
        mail_capacity: 0,
        passenger_capacity: 0,
        max_range: 0,
    };
}
