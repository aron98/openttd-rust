use super::*;
use crate::{TableLimits, TableTailPolicy};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
fn table(name: String, tail: Vec<u8>) -> TableChunk {
    TableChunk {
        id: *b"AIPL",
        kind: ChunkKind::Table,
        schema: TableSchema {
            fields: vec![FieldSchema {
                name,
                wire_type: 2,
                child: None,
            }],
        },
        records: std::collections::BTreeMap::from([(
            0,
            TableRecord {
                values: vec![WireValue::Unsigned(1)],
                tail,
            },
        )]),
        slots: 1,
        tail_policy: TableTailPolicy::PreserveScriptData,
        limits: TableLimits::default(),
    }
}
#[test]
fn counting_matches_encode_and_decode_resource_budgets() {
    for mut table in [
        table("x".repeat(1024), Vec::new()),
        table("value".into(), vec![7; 1000]),
    ] {
        for bytes in [64, 128, 512, 1024, 1100, 1500, 2048, 4096] {
            table.limits.max_bytes = bytes;
            let encoded = encode(&table);
            let expected = encoded.as_ref().ok().and_then(|chunk| {
                TableChunk::decode_with_limits(chunk, table.tail_policy, table.limits).ok()
            });
            let actual = validate(&table);
            assert_eq!(
                actual.is_ok(),
                expected.is_some(),
                "budget {bytes}: {actual:?}"
            );
            if let (Ok(count), Ok(chunk)) = (actual, encoded) {
                assert_eq!(count, chunk.body().len());
            }
        }
    }
}
#[test]
fn counting_rejects_decoder_only_name_and_tail_allocations() -> Result {
    for (mut table, bytes) in [
        (table("x".repeat(1024), Vec::new()), 1500),
        (table("value".into(), vec![7; 1000]), 1100),
    ] {
        table.limits.max_bytes = bytes;
        let encoded = encode(&table)?;
        assert!(TableChunk::decode_with_limits(&encoded, table.tail_policy, table.limits).is_err());
        assert!(validate(&table).is_err());
    }
    Ok(())
}
#[test]
fn counting_sink_retains_no_serialized_table_buffer() -> Result {
    let table = table("value".into(), vec![7; 1000]);
    let output = write(&table, false, false)?;
    assert!(output.bytes.is_empty());
    assert_eq!(output.length, encode(&table)?.body().len());
    for elements in [0, 1, 2, 3, 4, 16] {
        let mut candidate = table.clone();
        candidate.limits.max_elements = elements;
        assert_eq!(
            validate(&candidate).is_ok(),
            encode(&candidate)
                .and_then(|chunk| TableChunk::decode_with_limits(
                    &chunk,
                    candidate.tail_policy,
                    candidate.limits
                ))
                .is_ok()
        );
    }
    Ok(())
}
