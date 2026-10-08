//! Replays vectors produced by the instrumented upstream C++ executable.
use ottd_core::{CalendarDate, MapDimensions, Randomizer};
use serde_json::Value;

#[test]
fn primitives_match_upstream_vectors() {
    let source = std::env::var_os("OTTD_PRIMITIVES_JSON").map_or_else(
        || include_str!("fixtures/primitives.json").to_owned(),
        |path| std::fs::read_to_string(path).unwrap(),
    );
    let vectors: Value = serde_json::from_str(&source).unwrap();
    for vector in vectors.get("rng").unwrap().as_array().unwrap() {
        let state: [u32; 2] =
            serde_json::from_value(vector.get("initial_state").unwrap().clone()).unwrap();
        let mut rng = Randomizer::from_state(state);
        for step in vector.get("steps").unwrap().as_array().unwrap() {
            let value = match step.get("limit").unwrap().as_u64() {
                Some(limit) => rng.next_bounded(u32::try_from(limit).unwrap()),
                None => rng.next_u32(),
            };
            assert_eq!(
                u64::from(value),
                step.get("value").unwrap().as_u64().unwrap()
            );
            assert_eq!(
                serde_json::to_value(rng.state()).unwrap(),
                *step.get("state").unwrap()
            );
        }
    }
    for vector in vectors.get("calendar").unwrap().as_array().unwrap() {
        let year = i32::try_from(vector.get("year").unwrap().as_i64().unwrap()).unwrap();
        let month = u8::try_from(vector.get("month").unwrap().as_u64().unwrap()).unwrap();
        let day = u8::try_from(vector.get("day").unwrap().as_u64().unwrap()).unwrap();
        let date = CalendarDate::from_ymd(year, month, day).unwrap();
        assert_eq!(
            i64::from(date.raw()),
            vector.get("date").unwrap().as_i64().unwrap()
        );
        let expected = vector.get("roundtrip").unwrap();
        let (year, month, day) = date.ymd();
        assert_eq!(
            i64::from(year),
            expected.get("year").unwrap().as_i64().unwrap()
        );
        assert_eq!(
            u64::from(month),
            expected.get("month").unwrap().as_u64().unwrap()
        );
        assert_eq!(
            u64::from(day),
            expected.get("day").unwrap().as_u64().unwrap()
        );
    }
    let map = vectors.get("map").unwrap();
    let dimensions = MapDimensions::new(
        u32::try_from(map.get("width").unwrap().as_u64().unwrap()).unwrap(),
        u32::try_from(map.get("height").unwrap().as_u64().unwrap()).unwrap(),
    )
    .unwrap();
    for probe in map.get("coordinates").unwrap().as_array().unwrap() {
        let x = u32::try_from(probe.get("x").unwrap().as_u64().unwrap()).unwrap();
        let y = u32::try_from(probe.get("y").unwrap().as_u64().unwrap()).unwrap();
        let tile = dimensions.tile(x, y).unwrap();
        assert_eq!(
            u64::from(tile.raw()),
            probe.get("tile").unwrap().as_u64().unwrap()
        );
        assert_eq!(dimensions.coordinates(tile), Some((x, y)));
    }
}
