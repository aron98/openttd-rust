//! Terrain scheduler and validation regression tests.

use ottd_sim::{Landscape, Map, Tile};

const fn grass(m5: u8) -> Tile {
    Tile {
        tile_type: 0,
        height: 0,
        m1: 0,
        m2: 0,
        m3: 0,
        m4: 0,
        m5,
        m6: 0,
        m7: 0,
        m8: 0,
    }
}

#[test]
fn one_full_sweep_visits_every_tile_once() -> Result<(), Box<dyn std::error::Error>> {
    let map = Map {
        width: 64,
        height: 64,
        tiles: vec![grass(0); 4096],
    };
    let mut landscape = Landscape::new(map, 1)?;
    for tick in 1..=256 {
        landscape.advance(tick);
    }
    assert_eq!(landscape.cursor(), 1);
    assert!(landscape.map().tiles.iter().all(|tile| tile.m5 == 32));
    for tick in 257..=2048 {
        landscape.advance(tick);
    }
    assert!(landscape.map().tiles.iter().all(|tile| tile.m5 == 1));
    Ok(())
}

#[test]
fn rejects_unsupported_before_mutating() {
    for tile in [
        grass(12),
        Tile {
            tile_type: 0x10,
            ..grass(0)
        },
        Tile { m3: 16, ..grass(0) },
    ] {
        let map = Map {
            width: 64,
            height: 64,
            tiles: vec![tile; 4096],
        };
        assert!(Landscape::new(map, 1).is_err());
    }
    for cursor in [0, 4096] {
        assert!(
            Landscape::new(
                Map {
                    width: 64,
                    height: 64,
                    tiles: vec![grass(0); 4096]
                },
                cursor
            )
            .is_err()
        );
    }
}

#[test]
fn tile_zero_waits_for_tick_256() -> Result<(), Box<dyn std::error::Error>> {
    let mut map = Map {
        width: 64,
        height: 64,
        tiles: vec![grass(3); 4096],
    };
    *map.tiles.first_mut().ok_or("missing tile zero")? = grass(7 << 5);
    let mut landscape = Landscape::new(map, 1)?;
    for tick in 1..256 {
        landscape.advance(tick);
    }
    assert_eq!(landscape.map().tiles.first().ok_or("tile zero")?.m5, 224);
    landscape.advance(256);
    assert_eq!(landscape.map().tiles.first().ok_or("tile zero")?.m5, 1);
    Ok(())
}

#[test]
fn rectangular_sweeps_preserve_raw_fields_and_saturated_terrain()
-> Result<(), Box<dyn std::error::Error>> {
    for (width, height) in [(64, 128), (128, 64), (256, 64), (64, 256)] {
        let tiles = (0..width * height)
            .map(|i| Tile {
                tile_type: if i % 4 == 0 { 0x6f } else { 0x0f },
                height: 29,
                m1: 255,
                m2: 65535,
                m3: 239,
                m4: 231,
                m5: match i % 4 {
                    0 => 255,
                    1 => 3,
                    2 => 7,
                    _ => 11,
                },
                m6: 23,
                m7: 91,
                m8: 54321,
            })
            .collect();
        let map = Map {
            width,
            height,
            tiles,
        };
        let mut landscape = Landscape::new(map.clone(), 37)?;
        for tick in 1..=256 {
            landscape.advance(tick);
        }
        assert_eq!(landscape.cursor(), 37);
        assert_eq!(landscape.map(), &map);
    }
    Ok(())
}

#[test]
fn invalid_map_shapes_are_rejected() {
    for (width, height, count) in [
        (63, 64, 4032),
        (64, 32, 2048),
        (64, 64, 4095),
        (8192, 64, 0),
    ] {
        assert!(
            Landscape::new(
                Map {
                    width,
                    height,
                    tiles: vec![grass(0); count]
                },
                1
            )
            .is_err()
        );
    }
}
