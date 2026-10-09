//! Recursive version-362 table wire codec checks.
#![cfg(test)]
use ottd_save::{
    ChunkKind, Compression, Savegame, TableChunk, TableLimits, TableTailPolicy, WireValue,
};

fn gamma(value: usize) -> Vec<u8> {
    let [a, b, c, d] = u32::try_from(value).unwrap().to_be_bytes();
    match value {
        0..=127 => vec![d],
        128..=16_383 => vec![c | 0x80, d],
        16_384..=2_097_151 => vec![b | 0xc0, c, d],
        2_097_152..=268_435_455 => vec![a | 0xe0, b, c, d],
        _ => vec![240, a, b, c, d],
    }
}
fn save(id: impl AsRef<[u8]>, sparse: bool, header: &[u8], rows: &[Vec<u8>]) -> Savegame {
    let mut bytes = b"OTTN\x01\x6a\0\0".to_vec();
    bytes.extend(id.as_ref());
    bytes.push(if sparse { 4 } else { 3 });
    for row in std::iter::once(header).chain(rows.iter().map(Vec::as_slice)) {
        bytes.extend(gamma(row.len().checked_add(1).unwrap()));
        bytes.extend(row);
    }
    bytes.extend([0; 5]);
    Savegame::decode(&bytes, 1024 * 1024).unwrap()
}
fn nested() -> Savegame {
    // Root x:u32, children:struct list, flag:u8; child n:i16; headers follow terminator.
    save(
        b"TEST",
        true,
        &[6, 1, b'x', 27, 1, b'c', 2, 1, b'f', 0, 3, 1, b'n', 0],
        &[
            vec![0, 0, 0, 0, 1, 2, 255, 254, 0, 7, 9],
            [gamma(300), vec![0, 0, 0, 2, 0, 8]].concat(),
        ],
    )
}
#[test]
fn decodes_recursive_sparse_rows_and_reencodes_exactly() {
    // Given nested native-style framing with sparse ID 300.
    let save = nested();
    let chunk = save.chunks().first().unwrap();
    // When decoded and encoded.
    let table = TableChunk::decode(chunk, TableTailPolicy::Reject).unwrap();
    // Then child schema order, signed nested values, and exact framing survive.
    assert_eq!(
        table.records().keys().copied().collect::<Vec<_>>(),
        vec![0, 300]
    );
    assert_eq!(
        table
            .schema()
            .fields()
            .get(1)
            .unwrap()
            .child()
            .unwrap()
            .fields()
            .first()
            .unwrap()
            .name(),
        "n"
    );
    let WireValue::Structs(rows) = table.records().get(&0).unwrap().values().get(1).unwrap() else {
        panic!()
    };
    assert_eq!(rows.first().unwrap().values(), &[WireValue::Signed(-2)]);
    assert_eq!(table.encode().unwrap().body(), chunk.body());
}
#[test]
fn mutation_survives_container_replacement_without_changing_siblings() {
    // Given an existing sparse table.
    let mut save = nested();
    let mut table =
        TableChunk::decode(save.chunks().first().unwrap(), TableTailPolicy::Reject).unwrap();
    let sibling = table.records().get(&0).unwrap().clone();
    // When a single value changes and the full container is serialized and parsed.
    *table
        .records_mut()
        .get_mut(&300)
        .unwrap()
        .values_mut()
        .first_mut()
        .unwrap() = WireValue::Unsigned(98765);
    save.replace_chunk(table.encode().unwrap()).unwrap();
    let reloaded = Savegame::decode(&save.encode(Compression::None).unwrap(), 1024 * 1024).unwrap();
    let table =
        TableChunk::decode(reloaded.chunks().first().unwrap(), TableTailPolicy::Reject).unwrap();
    // Then the exact edit and untouched sibling remain.
    assert_eq!(
        table.records().get(&300).unwrap().values().first(),
        Some(&WireValue::Unsigned(98765))
    );
    assert_eq!(table.records().get(&0).unwrap(), &sibling);
}
#[test]
fn refuses_integer_overflow_at_encoding_boundary() {
    // Given an unsigned byte field.
    let save = save(b"TEST", false, &[2, 1, b'x', 0], &[vec![0]]);
    let mut table =
        TableChunk::decode(save.chunks().first().unwrap(), TableTailPolicy::Reject).unwrap();
    // When a value outside its descriptor width is inserted, then encoding rejects it.
    *table
        .records_mut()
        .get_mut(&0)
        .unwrap()
        .values_mut()
        .first_mut()
        .unwrap() = WireValue::Unsigned(256);
    assert!(table.encode().is_err());
}
#[test]
fn script_tails_require_explicit_named_policy() {
    // Given identical trailing data in a script and ordinary chunk.
    for id in [b"AIPL", b"GSDT", b"TEST"] {
        let save = save(id, false, &[2, 1, b'x', 0], &[vec![1, 0xfe, 0xff]]);
        let chunk = save.chunks().first().unwrap();
        // When strict or script-specific tail policy is selected.
        assert!(TableChunk::decode(chunk, TableTailPolicy::Reject).is_err());
        let result = TableChunk::decode(chunk, TableTailPolicy::PreserveScriptData);
        // Then only the two named chunks permit exact opaque tail preservation.
        if id == b"TEST" {
            assert!(result.is_err());
        } else {
            let table = result.unwrap();
            assert_eq!(table.records().get(&0).unwrap().tail(), &[0xfe, 0xff]);
            assert_eq!(table.encode().unwrap().body(), chunk.body());
        }
    }
}
#[test]
fn rejects_duplicate_sparse_ids_and_truncated_nested_data() {
    // Given invalid records, when decoded, then each is rejected.
    for input in [
        save(b"TEST", true, &[2, 1, b'x', 0], &[vec![0, 1], vec![0, 2]]),
        save(
            b"TEST",
            false,
            &[27, 1, b'x', 0, 6, 1, b'y', 0],
            &[vec![1, 0]],
        ),
    ] {
        assert!(
            TableChunk::decode(input.chunks().first().unwrap(), TableTailPolicy::Reject).is_err()
        );
    }
}
#[test]
fn element_budget_prevents_empty_struct_count_amplification() {
    // Given a tiny frame requesting many empty child records.
    let save = save(b"TEST", false, &[27, 1, b'x', 0, 0], &[gamma(10000)]);
    // When decoded with a small node budget, then amplification is rejected.
    assert!(
        TableChunk::decode_with_limits(
            save.chunks().first().unwrap(),
            TableTailPolicy::Reject,
            TableLimits {
                max_elements: 32,
                ..TableLimits::default()
            }
        )
        .is_err()
    );
}
#[test]
fn native_v362_tables_reencode_without_any_byte_change() {
    // Given both committed native version-362 fixtures.
    for bytes in [
        include_bytes!("../../../fixtures/generated-v362.sav").as_slice(),
        include_bytes!("../../../fixtures/contracts/modded-v362.sav").as_slice(),
    ] {
        let save = Savegame::decode(bytes, 256 * 1024 * 1024).unwrap();
        // When each table is decoded and reencoded using named script policy only where needed.
        for chunk in save
            .chunks()
            .iter()
            .filter(|chunk| matches!(chunk.kind(), ChunkKind::Table | ChunkKind::SparseTable))
        {
            let policy = if matches!(&chunk.id(), b"AIPL" | b"GSDT") {
                TableTailPolicy::PreserveScriptData
            } else {
                TableTailPolicy::Reject
            };
            let table = TableChunk::decode(chunk, policy)
                .unwrap_or_else(|e| panic!("{:?}: {e}", chunk.id()));
            // Then all header, index, data, and script-tail bytes match the independent fixture.
            assert_eq!(
                table.encode().unwrap().body(),
                chunk.body(),
                "{:?}",
                chunk.id()
            );
        }
    }
}

#[test]
fn preserves_ordinary_holes_and_trailing_empty_slots() {
    // Given rows at zero and two, followed by another empty slot.
    let save = save(
        b"TEST",
        false,
        &[2, 1, b'x', 0],
        &[vec![1], vec![], vec![2], vec![]],
    );
    let chunk = save.chunks().first().unwrap();
    // When the table is decoded and encoded.
    let table = TableChunk::decode(chunk, TableTailPolicy::Reject).unwrap();
    // Then implicit indices and the trailing hole survive.
    assert_eq!(
        table.records().keys().copied().collect::<Vec<_>>(),
        vec![0, 2]
    );
    assert_eq!(table.encode().unwrap().body(), chunk.body());
}

#[test]
fn preserves_raw_string_bytes_and_reference_array_words() {
    // Given non-UTF-8 native string bytes and raw nullable reference words.
    let save = save(
        b"TEST",
        false,
        &[26, 1, b's', 22, 1, b'r', 0],
        &[vec![
            2, 0xff, 0xfe, 3, 0, 0, 0, 0, 0, 0, 0, 1, 255, 255, 255, 255,
        ]],
    );
    let chunk = save.chunks().first().unwrap();
    // When interpreted only as wire values.
    let table = TableChunk::decode(chunk, TableTailPolicy::Reject).unwrap();
    // Then the codec neither normalizes text nor decrements pool references.
    assert_eq!(
        table.records().get(&0).unwrap().values(),
        &[
            WireValue::Bytes(vec![0xff, 0xfe]),
            WireValue::Array(vec![
                WireValue::Unsigned(0),
                WireValue::Unsigned(1),
                WireValue::Unsigned(u64::from(u32::MAX))
            ])
        ]
    );
    assert_eq!(table.encode().unwrap().body(), chunk.body());
}

#[test]
fn rejects_invalid_schema_and_value_shapes() {
    // Given duplicate fields, unknown flags, missing child headers or incomplete rows.
    let inputs = [
        save(b"TEST", false, &[2, 1, b'x', 2, 1, b'x', 0], &[vec![1, 2]]),
        save(b"TEST", false, &[34, 1, b'x', 0], &[vec![1]]),
        save(b"TEST", false, &[27, 1, b'x', 0], &[vec![0]]),
        save(b"TEST", false, &[2, 1, b'x', 0, 0], &[vec![1]]),
        save(b"TEST", false, &[6, 1, b'x', 0], &[vec![1]]),
    ];
    // When each is decoded, then none crosses the typed boundary.
    for input in inputs {
        assert!(
            TableChunk::decode(input.chunks().first().unwrap(), TableTailPolicy::Reject).is_err()
        );
    }
}

#[test]
fn rejects_excessive_schema_depth_and_wire_size() {
    // Given a two-level schema and limits excluding that depth or its bytes.
    let save = nested();
    let chunk = save.chunks().first().unwrap();
    // When limits are applied, then decoding fails before unbounded allocation.
    for limits in [
        TableLimits {
            max_depth: 1,
            ..TableLimits::default()
        },
        TableLimits {
            max_bytes: 4,
            ..TableLimits::default()
        },
    ] {
        assert!(TableChunk::decode_with_limits(chunk, TableTailPolicy::Reject, limits).is_err());
    }
}

#[test]
fn rejects_nested_edited_shape_mismatch() {
    // Given a child record whose descriptor requires an i16.
    let save = nested();
    let mut table =
        TableChunk::decode(save.chunks().first().unwrap(), TableTailPolicy::Reject).unwrap();
    // When the child is changed to an unsigned value, then encoding fails.
    let WireValue::Structs(rows) = table
        .records_mut()
        .get_mut(&0)
        .unwrap()
        .values_mut()
        .get_mut(1)
        .unwrap()
    else {
        panic!()
    };
    *rows.first_mut().unwrap().values_mut().first_mut().unwrap() = WireValue::Unsigned(1);
    assert!(table.encode().is_err());
}

proptest::proptest! {
    #[test]
    fn all_integer_widths_preserve_bits(value in proptest::prelude::any::<u64>()) {
        // Given the same randomized bits represented at every native scalar width.
        let raw = value.to_be_bytes();
        for (kind, width) in [(1,1),(2,1),(3,2),(4,2),(5,4),(6,4),(7,8),(8,8),(9,2)] {
            let row = raw.get(8_usize.checked_sub(width).unwrap()..).unwrap().to_vec();
            let save = save(b"TEST", false, &[kind, 1, b'x', 0], &[row]);
            let chunk = save.chunks().first().unwrap();
            // When decoded and encoded, then all signed/unsigned bits remain exact.
            let table = TableChunk::decode(chunk, TableTailPolicy::Reject).unwrap();
            let encoded = table.encode().unwrap();
            proptest::prop_assert_eq!(encoded.body(), chunk.body());
        }
    }
}
