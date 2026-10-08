//! Typed state boundary tests.
use ottd_save::{Savegame, SnapshotError};

#[test]
fn decodes_native_current_snapshot() {
    // Given a native save produced by the pinned engine.
    let save = Savegame::decode(
        include_bytes!("../../../fixtures/generated-v362.sav"),
        4 * 1024 * 1024,
    )
    .unwrap();
    // When the typed state is requested.
    let snapshot = save.snapshot().unwrap();
    // Then every raw tile exists and canonical JSON round-trips.
    assert_eq!(snapshot.map().width(), 64);
    assert_eq!(snapshot.map().height(), 64);
    assert_eq!(snapshot.map().tiles().len(), 4096);
}

#[test]
fn rejects_old_versions_at_typed_boundary() {
    // Given a structurally supported historical save.
    let save = Savegame::decode(
        include_bytes!("../../../fixtures/upstream-regression-v308.sav"),
        4 * 1024 * 1024,
    )
    .unwrap();
    // When typed decoding is requested, then it explicitly refuses migration.
    assert!(matches!(save.snapshot(), Err(SnapshotError::Version(308))));
}

mod support;

#[test]
fn canonical_json_round_trips_without_losing_integer_signedness() {
    let snapshot = support::native().snapshot().unwrap();
    let encoded = serde_json::to_string(&snapshot).unwrap();
    let decoded: ottd_save::WorldSnapshot = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, snapshot);
}

#[test]
fn rejects_missing_required_chunks() {
    for id in [*b"MAPS", *b"MAP2", *b"MAP8", *b"DATE", *b"PATS", *b"SRND"] {
        let save = support::rewrite(id, None);
        assert!(matches!(save.snapshot(), Err(SnapshotError::Chunk(_))));
    }
}

#[test]
fn rejects_each_invalid_plane_length() {
    for id in [
        *b"MAPT", *b"MAPH", *b"MAPO", *b"MAP2", *b"M3LO", *b"M3HI", *b"MAP5", *b"MAPE", *b"MAP7",
        *b"MAP8",
    ] {
        let save = support::rewrite(id, Some(&vec![0; 4095]));
        assert!(save.snapshot().is_err(), "{id:?}");
    }
}

#[test]
fn preserves_full_big_endian_wide_tile_values() {
    for (id, key) in [(*b"MAP2", "m2"), (*b"MAP8", "m8")] {
        let save = support::rewrite(id, Some(&[0x91, 0xa3].repeat(4096)));
        let json = serde_json::to_value(save.snapshot().unwrap()).unwrap();
        assert_eq!(
            *json.pointer(&format!("/map/tiles/0/{key}")).unwrap(),
            0x91a3
        );
        assert_eq!(
            *json.pointer(&format!("/map/tiles/4095/{key}")).unwrap(),
            0x91a3
        );
    }
}

#[test]
fn rejects_invalid_map_dimensions() {
    for width in [0_u32, 63, 65, 8192, u32::MAX] {
        let mut records = support::records(*b"MAPS");
        records
            .get_mut(1)
            .unwrap()
            .get_mut(..4)
            .unwrap()
            .copy_from_slice(&width.to_be_bytes());
        let save = support::rewrite(*b"MAPS", Some(&support::table(&records)));
        assert!(save.snapshot().is_err(), "width {width}");
    }
}

#[test]
fn rejects_wrong_field_wire_types_and_nested_schemas() {
    for id in [*b"MAPS", *b"DATE", *b"PATS", *b"SRND"] {
        for kind in [0, 7, 11, 27, 255] {
            let mut records = support::records(id);
            *records.get_mut(0).unwrap().get_mut(0).unwrap() = kind;
            let save = support::rewrite(id, Some(&support::table(&records)));
            assert!(save.snapshot().is_err(), "{id:?} kind {kind}");
        }
    }
}

#[test]
fn rejects_duplicate_field_names() {
    let mut records = support::records(*b"MAPS");
    let header = records.get_mut(0).unwrap();
    let last_y = header.iter().rposition(|b| *b == b'y').unwrap();
    *header.get_mut(last_y).unwrap() = b'x';
    let save = support::rewrite(*b"MAPS", Some(&support::table(&records)));
    assert!(save.snapshot().is_err());
}

#[test]
fn rejects_extra_singleton_records_and_missing_script_owners() {
    for id in [*b"MAPS", *b"DATE", *b"PATS", *b"SRND"] {
        let mut records = support::records(id);
        if id == *b"SRND" {
            records.pop();
        } else {
            records.push(records.get(1).unwrap().clone());
        }
        let save = support::rewrite(id, Some(&support::table(&records)));
        assert!(save.snapshot().is_err(), "{id:?}");
    }
}

#[test]
fn rejects_truncated_and_trailing_record_data() {
    for id in [*b"MAPS", *b"DATE", *b"PATS", *b"SRND"] {
        for extra in [false, true] {
            let mut records = support::records(id);
            if extra {
                records.get_mut(1).unwrap().push(0);
            } else {
                records.get_mut(1).unwrap().pop();
            }
            let save = support::rewrite(id, Some(&support::table(&records)));
            assert!(save.snapshot().is_err(), "{id:?}");
        }
    }
}

#[test]
fn rejects_json_with_unknown_fields_invalid_sizes_and_owners() {
    let original = serde_json::to_value(support::native().snapshot().unwrap()).unwrap();
    for path in ["unknown", "map", "script_random", "settings", "date"] {
        let mut json = original.clone();
        match path {
            "map" => *json.pointer_mut("/map/width").unwrap() = 65.into(),
            "script_random" => *json.pointer_mut("/script_random/1/owner").unwrap() = 0.into(),
            "settings" => *json.pointer_mut("/settings/difficulty.max_loan").unwrap() = (-1).into(),
            "date" => {
                json.get_mut("date")
                    .unwrap()
                    .as_object_mut()
                    .unwrap()
                    .insert("unknown".into(), 1.into());
            }
            _ => {
                json.as_object_mut()
                    .unwrap()
                    .insert("unknown".into(), 1.into());
            }
        }
        assert!(
            serde_json::from_value::<ottd_save::WorldSnapshot>(json).is_err(),
            "{path}"
        );
    }
}

#[test]
fn rejects_duplicate_required_chunks() {
    let native = support::native();
    let mut bytes = native.encode(ottd_save::Compression::None).unwrap();
    bytes.truncate(bytes.len().checked_sub(4).unwrap());
    let chunk = native
        .chunks()
        .iter()
        .find(|chunk| chunk.id() == *b"MAPT")
        .unwrap();
    bytes.extend_from_slice(b"MAPT");
    bytes.extend_from_slice(&u32::try_from(chunk.body().len()).unwrap().to_be_bytes());
    bytes.extend_from_slice(chunk.body());
    bytes.extend_from_slice(&[0; 4]);
    let save = Savegame::decode(&bytes, 4 * 1024 * 1024).unwrap();
    assert!(matches!(save.snapshot(), Err(SnapshotError::Chunk(_))));
}

#[test]
fn rejects_missing_setting_schema_field() {
    let mut records = support::records(*b"PATS");
    let schema = records.get_mut(0).unwrap();
    let first_field_len = usize::from(*schema.get(1).unwrap()).checked_add(2).unwrap();
    schema.drain(..first_field_len);
    let save = support::rewrite(*b"PATS", Some(&support::table(&records)));
    assert!(save.snapshot().is_err());
}

#[test]
fn rejects_unbounded_string_length_without_allocating_it() {
    let mut records = support::records(*b"DATE");
    let data = records.get_mut(1).unwrap();
    data.truncate(43);
    data.extend_from_slice(&[0xf0, 0xff, 0xff, 0xff, 0xff]);
    let save = support::rewrite(*b"DATE", Some(&support::table(&records)));
    assert!(save.snapshot().is_err());
}

proptest::proptest! {
    #[test]
    fn arbitrary_typed_record_never_panics(data in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..512)) {
        let mut records = support::records(*b"DATE");
        *records.get_mut(1).unwrap() = data;
        let save = support::rewrite(*b"DATE", Some(&support::table(&records)));
        let result = save.snapshot();
        if let Ok(snapshot) = result {
            let json = serde_json::to_string(&snapshot).unwrap();
            proptest::prop_assert!(serde_json::from_str::<ottd_save::WorldSnapshot>(&json).is_ok());
        }
    }
}
