use super::{
    CargoLabelSource, CargoSpec, Climate, ContentError, EngineInfo, EngineSpec, VehicleSpec,
    aircraft_data, engine_data::ENGINES, rail_data, road_data, ship_data,
};

pub(super) struct EngineSeed {
    pub intro: i32,
    pub decay: u8,
    pub life: i32,
    pub base_life: i32,
    pub cargo: CargoLabelSource,
    pub climates: u8,
    pub misc: u8,
    pub load: u8,
}

pub(super) fn raw_original(kind: usize, local: u16, id: u16) -> Option<EngineSpec> {
    let index = usize::from(local);
    let (offset, count, vehicle): (usize, usize, VehicleSpec) = match kind {
        0 => (
            0,
            116,
            VehicleSpec::Rail(
                rail_data::VEHICLES
                    .get(index)
                    .copied()
                    .unwrap_or(super::RailSpec::DEFAULT),
            ),
        ),
        1 => (
            116,
            88,
            VehicleSpec::Road(
                road_data::VEHICLES
                    .get(index)
                    .copied()
                    .unwrap_or(super::RoadSpec::DEFAULT),
            ),
        ),
        2 => (
            204,
            11,
            VehicleSpec::Ship(
                ship_data::VEHICLES
                    .get(index)
                    .copied()
                    .unwrap_or(super::ShipSpec::DEFAULT),
            ),
        ),
        3 => (
            215,
            41,
            VehicleSpec::Aircraft(
                aircraft_data::VEHICLES
                    .get(index)
                    .copied()
                    .unwrap_or(super::AircraftSpec::DEFAULT),
            ),
        ),
        _ => return None,
    };
    let mut info = EngineInfo {
        base_intro: 0,
        lifelength: 0,
        base_life: 255,
        decay_speed: 0,
        load_amount: 0,
        climates: 0,
        cargo_type: 255,
        cargo_label: CargoLabelSource::Fixed(if kind == 3 {
            u32::MAX
        } else {
            u32::from_be_bytes(*b"PASS")
        }),
        refit_mask: 0,
        refit_cost: 0,
        misc_flags: 0,
        callback_mask: 0,
        retire_early: 0,
        extra_flags: 0,
        cargo_age_period: 185,
        variant_id: u16::MAX,
    };
    if index < count {
        let seed = ENGINES.get(offset.checked_add(index)?)?;
        info.base_intro = 701_265_i32.checked_add(seed.intro)?;
        info.lifelength = seed.life;
        info.base_life = if matches!(vehicle, VehicleSpec::Rail(rail) if rail.railveh_type == 2) {
            255
        } else {
            seed.base_life
        };
        info.decay_speed = seed.decay;
        info.load_amount = seed.load;
        info.climates = seed.climates;
        info.cargo_label = seed.cargo;
        info.refit_cost = 8;
        info.misc_flags = seed.misc;
    }
    Some(EngineSpec {
        id,
        local_id: local,
        info,
        vehicle,
    })
}

pub(super) fn initialize(
    climate: Climate,
    cargo: &[CargoSpec],
) -> Result<Vec<EngineSpec>, ContentError> {
    let vehicles = rail_data::VEHICLES
        .iter()
        .copied()
        .map(VehicleSpec::Rail)
        .chain(road_data::VEHICLES.iter().copied().map(VehicleSpec::Road))
        .chain(ship_data::VEHICLES.iter().copied().map(VehicleSpec::Ship))
        .chain(
            aircraft_data::VEHICLES
                .iter()
                .copied()
                .map(VehicleSpec::Aircraft),
        );
    ENGINES
        .iter()
        .zip(vehicles)
        .enumerate()
        .map(|(id, (seed, mut vehicle))| {
            let offset = match vehicle {
                VehicleSpec::Rail(_) => 0,
                VehicleSpec::Road(_) => 116,
                VehicleSpec::Ship(_) => 204,
                VehicleSpec::Aircraft(_) => 215,
            };
            let id = u16::try_from(id).map_err(|_| ContentError::Arithmetic)?;
            let label = resolve_label(seed.cargo, cargo);
            let mut cargo_type = cargo
                .iter()
                .position(|c| c.bitnum != 255 && c.label == label)
                .map(u8::try_from)
                .transpose()
                .map_err(|_| ContentError::Arithmetic)?
                .unwrap_or(255);
            let (allowed, disallowed, specialized) = refit_classes(&vehicle, label, climate);
            let mut refit_mask = 0_u64;
            for (slot, c) in cargo.iter().enumerate().filter(|(_, c)| c.bitnum != 255) {
                if c.classes & allowed != 0
                    && c.classes & disallowed == 0
                    && (!specialized || usize::from(cargo_type) == slot)
                {
                    refit_mask |= 1_u64
                        .checked_shl(u32::try_from(slot).map_err(|_| ContentError::Arithmetic)?)
                        .ok_or(ContentError::Arithmetic)?;
                }
            }
            if allowed != 0 && cargo_type != 255 && refit_mask & (1_u64 << cargo_type) == 0 {
                cargo_type = 255;
            }
            if cargo_type == 255 && refit_mask != 0 {
                cargo_type = u8::try_from(refit_mask.trailing_zeros())
                    .map_err(|_| ContentError::Arithmetic)?;
            }
            match &mut vehicle {
                VehicleSpec::Ship(ship) => ship.old_refittable = true,
                VehicleSpec::Rail(_) | VehicleSpec::Road(_) | VehicleSpec::Aircraft(_) => {}
            }
            let wagon = matches!(vehicle, VehicleSpec::Rail(rail) if rail.railveh_type == 2);
            Ok(EngineSpec {
                id,
                local_id: id.checked_sub(offset).ok_or(ContentError::Arithmetic)?,
                info: EngineInfo {
                    base_intro: 701_265_i32
                        .checked_add(seed.intro)
                        .ok_or(ContentError::Arithmetic)?,
                    lifelength: seed.life,
                    base_life: if wagon { 255 } else { seed.base_life },
                    decay_speed: seed.decay,
                    load_amount: seed.load,
                    climates: if cargo_type == 255 { 0 } else { seed.climates },
                    cargo_type,
                    cargo_label: seed.cargo,
                    refit_mask,
                    refit_cost: 8,
                    misc_flags: seed.misc,
                    callback_mask: 0,
                    retire_early: 0,
                    extra_flags: 0,
                    cargo_age_period: 185,
                    variant_id: 65535,
                },
                vehicle,
            })
        })
        .collect()
}

fn resolve_label(source: CargoLabelSource, cargo: &[CargoSpec]) -> u32 {
    let labels: &[u32] = match source {
        CargoLabelSource::Fixed(label) => return label,
        CargoLabelSource::LivestockFruit => {
            &[u32::from_be_bytes(*b"LVST"), u32::from_be_bytes(*b"FRUT")]
        }
        CargoLabelSource::GrainWheatMaize => &[
            u32::from_be_bytes(*b"GRAI"),
            u32::from_be_bytes(*b"WHEA"),
            u32::from_be_bytes(*b"MAIZ"),
        ],
        CargoLabelSource::ValuablesGoldDiamonds => &[
            u32::from_be_bytes(*b"VALU"),
            u32::from_be_bytes(*b"GOLD"),
            u32::from_be_bytes(*b"DIAM"),
        ],
    };
    labels
        .iter()
        .copied()
        .find(|label| cargo.iter().any(|c| c.bitnum != 255 && c.label == *label))
        .unwrap_or(u32::MAX)
}

fn refit_classes(vehicle: &VehicleSpec, label: u32, climate: Climate) -> (u16, u16, bool) {
    match vehicle {
        VehicleSpec::Aircraft(_) => (15, 64, false),
        VehicleSpec::Ship(_) => match label.to_be_bytes() {
            [b'P', b'A', b'S', b'S'] => (1, 0, false),
            [b'O', b'I', b'L', b'_'] => (64, 0, false),
            _ => {
                if climate == Climate::Toyland {
                    (126, 1, false)
                } else {
                    (62, 65, false)
                }
            }
        },
        VehicleSpec::Rail(rail) if rail.capacity == 0 => (0, 0, false),
        VehicleSpec::Rail(rail) if rail.railveh_type != 2 => (127, 0, false),
        VehicleSpec::Rail(_) | VehicleSpec::Road(_) => {
            let (allow, deny) = match label.to_be_bytes() {
                [b'P', b'A', b'S', b'S'] => (1, 0),
                [b'M', b'A', b'I', b'L'] => {
                    if climate == Climate::Toyland {
                        (10, 64)
                    } else {
                        (2, 0)
                    }
                }
                [b'V', b'A', b'L', b'U'] if climate != Climate::Toyland => (8, 64),
                [b'C', b'O', b'A', b'L']
                    if matches!(climate, Climate::Temperate | Climate::Arctic) =>
                {
                    (16, 0)
                }
                [b'C', b'O', b'R', b'E'] if climate == Climate::Tropic => (16, 0),
                [b'S', b'U', b'G', b'R'] if climate == Climate::Toyland => (16, 0),
                [b'O', b'I', b'L', b'_'] if climate != Climate::Toyland => (64, 0),
                [b'C', b'O', b'L', b'A'] if climate == Climate::Toyland => (64, 0),
                [b'G', b'O', b'O', b'D'] if climate != Climate::Toyland => (
                    36,
                    if climate == Climate::Temperate {
                        65
                    } else {
                        193
                    },
                ),
                [b'F', b'O', b'O', b'D']
                    if matches!(climate, Climate::Arctic | Climate::Tropic) =>
                {
                    (128, 0)
                }
                [b'S', b'W', b'E', b'T'] if climate == Climate::Toyland => (36, 65),
                _ => (0, 0),
            };
            (allow, deny, true)
        }
    }
}
