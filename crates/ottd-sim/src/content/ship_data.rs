// SPDX-License-Identifier: GPL-2.0-only
// Ported from OpenTTD 14ec60f248547d4d062a1160f0fc26d742319888 src/table/engines.h.
use super::ShipSpec;
pub(super) const VEHICLES: [ShipSpec; 11] = [
    ShipSpec {
        image_index: 1,
        cost_factor: 160,
        acceleration: 1,
        max_speed: 48,
        capacity: 220,
        running_cost: 140,
        sfx: 4,
        old_refittable: false,
        ..ShipSpec::DEFAULT
    },
    ShipSpec {
        image_index: 1,
        cost_factor: 176,
        acceleration: 1,
        max_speed: 80,
        capacity: 350,
        running_cost: 125,
        sfx: 4,
        old_refittable: false,
        ..ShipSpec::DEFAULT
    },
    ShipSpec {
        image_index: 2,
        cost_factor: 96,
        acceleration: 1,
        max_speed: 64,
        capacity: 100,
        running_cost: 90,
        sfx: 5,
        old_refittable: false,
        ..ShipSpec::DEFAULT
    },
    ShipSpec {
        image_index: 2,
        cost_factor: 112,
        acceleration: 1,
        max_speed: 128,
        capacity: 130,
        running_cost: 80,
        sfx: 5,
        old_refittable: false,
        ..ShipSpec::DEFAULT
    },
    ShipSpec {
        image_index: 3,
        cost_factor: 148,
        acceleration: 1,
        max_speed: 224,
        capacity: 100,
        running_cost: 190,
        sfx: 5,
        old_refittable: false,
        ..ShipSpec::DEFAULT
    },
    ShipSpec {
        image_index: 2,
        cost_factor: 96,
        acceleration: 1,
        max_speed: 64,
        capacity: 100,
        running_cost: 90,
        sfx: 5,
        old_refittable: false,
        ..ShipSpec::DEFAULT
    },
    ShipSpec {
        image_index: 2,
        cost_factor: 112,
        acceleration: 1,
        max_speed: 128,
        capacity: 130,
        running_cost: 80,
        sfx: 5,
        old_refittable: false,
        ..ShipSpec::DEFAULT
    },
    ShipSpec {
        image_index: 0,
        cost_factor: 128,
        acceleration: 1,
        max_speed: 48,
        capacity: 160,
        running_cost: 150,
        sfx: 4,
        old_refittable: true,
        ..ShipSpec::DEFAULT
    },
    ShipSpec {
        image_index: 0,
        cost_factor: 144,
        acceleration: 1,
        max_speed: 80,
        capacity: 190,
        running_cost: 113,
        sfx: 4,
        old_refittable: true,
        ..ShipSpec::DEFAULT
    },
    ShipSpec {
        image_index: 0,
        cost_factor: 128,
        acceleration: 1,
        max_speed: 48,
        capacity: 160,
        running_cost: 150,
        sfx: 4,
        old_refittable: true,
        ..ShipSpec::DEFAULT
    },
    ShipSpec {
        image_index: 0,
        cost_factor: 144,
        acceleration: 1,
        max_speed: 80,
        capacity: 190,
        running_cost: 113,
        sfx: 4,
        old_refittable: true,
        ..ShipSpec::DEFAULT
    },
];
