use super::{Landscape, LandscapeError, Map, TileLoop};

#[test]
fn invalid_owned_scheduler_cursor_returns_error_without_skipping() {
    let mut landscape = Landscape {
        map: Map {
            width: 64,
            height: 64,
            tiles: Vec::new(),
        },
        scheduler: TileLoop {
            cursor: 1,
            feedback: 0xD8F,
            visits: 1,
        },
    };
    assert_eq!(landscape.advance(1), Err(LandscapeError::Cursor));
    assert_eq!(landscape.cursor(), 1);
    assert!(landscape.map().tiles.is_empty());
}
