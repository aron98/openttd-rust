//! The closed terrain domain excludes sea-level neighbors of flooding void tiles.
use ottd_sim::{Landscape, Map, Tile};
fn map() -> Map {
    Map {
        width: 64,
        height: 64,
        tiles: vec![
            Tile {
                tile_type: 0,
                height: 1,
                m1: 0,
                m2: 0,
                m3: 0,
                m4: 0,
                m5: 0,
                m6: 0,
                m7: 0,
                m8: 0
            };
            4096
        ],
    }
}
#[test]
fn direct_and_diagonal_neighbors_cannot_flood() -> Result<(), Box<dyn std::error::Error>> {
    for (void, low) in [(65, 66), (65, 130), (65, 195), (4095, 4094), (0, 1)] {
        let mut map = map();
        map.tiles.get_mut(void).ok_or("void")?.tile_type = 0x70;
        map.tiles.get_mut(low).ok_or("low")?.height = 0;
        assert!(Landscape::new(map, 1).is_err(), "void {void} low {low}");
    }
    Ok(())
}
#[test]
fn void_corners_count_but_distant_interior_sea_level_is_supported()
-> Result<(), Box<dyn std::error::Error>> {
    let mut map = map();
    map.tiles.get_mut(65).ok_or("void")?.tile_type = 0x70;
    map.tiles.get_mut(65).ok_or("void")?.height = 0;
    assert!(Landscape::new(map.clone(), 1).is_err());
    map.tiles.get_mut(65).ok_or("void")?.height = 1;
    map.tiles.get_mut(1000).ok_or("interior")?.height = 0;
    assert!(Landscape::new(map, 1).is_ok());
    Ok(())
}
