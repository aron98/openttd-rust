//! Wire-format and corruption tests independent of the upstream executable.

use ottd_save::{Compression, Savegame};
use proptest::prelude::*;

const LIMIT: usize = 1024 * 1024;
const FORMATS: [Compression; 4] = [
    Compression::None,
    Compression::Zlib,
    Compression::Lzma,
    Compression::Lzo,
];

fn container(payload: &[u8]) -> Vec<u8> {
    let mut bytes = b"OTTN\x01\x6a\0\0".to_vec();
    bytes.extend_from_slice(payload);
    bytes
}

fn sample() -> Vec<u8> {
    container(b"MAPS\0\0\0\x04\0\0\0\x40ARRY\x01\x01\x04abc\0SPAR\x02\x05\x81\x00xy\0TABL\x03\x02\0\x03hi\0STBL\x04\x02\0\x04\x05ok\0\0\0\0\0")
}

#[test]
fn preserves_every_chunk_kind_when_rewritten_uncompressed() {
    let original = sample();
    let rewritten = Savegame::decode(&original, LIMIT)
        .unwrap()
        .encode(Compression::None)
        .unwrap();
    assert_eq!(rewritten, original);
}

#[test]
fn retains_state_when_converted_between_all_compression_formats() {
    let original = sample();
    let parsed = Savegame::decode(&original, LIMIT).unwrap();
    for format in FORMATS {
        let encoded = parsed.encode(format).unwrap();
        let decoded = Savegame::decode(&encoded, LIMIT).unwrap();
        assert_eq!(decoded.encode(Compression::None).unwrap(), original);
    }
}

#[test]
fn rejects_truncation_at_every_position() {
    let complete = sample();
    for end in 0..complete.len() {
        let truncated = complete.get(..end).unwrap();
        assert!(Savegame::decode(truncated, LIMIT).is_err(), "offset {end}");
    }
}

#[test]
fn rejects_corrupt_chunk_framing() {
    let malformed: &[&[u8]] = &[
        b"MAPS\0\0\0\x20short",
        b"ARRY\x01\xf8",
        b"SPAR\x02\x01\0\0\0\0\0",
        b"SPAR\x02\x02\x80\0\0\0\0\0",
        b"TABL\x03\0\0\0\0\0",
        b"MAPS\x05\0\0\0\0",
        b"\0\0\0\0trailing",
    ];
    for payload in malformed {
        assert!(Savegame::decode(&container(payload), LIMIT).is_err());
    }
}

#[test]
fn rejects_future_and_patchpack_versions() {
    for version in [363_u16, 220, 286, u16::MAX] {
        let mut bytes = sample();
        bytes
            .get_mut(4..6)
            .unwrap()
            .copy_from_slice(&version.to_be_bytes());
        assert!(Savegame::decode(&bytes, LIMIT).is_err());
    }
}

#[test]
fn preserves_historical_version_bytes_without_upgrading() {
    let mut bytes = sample();
    bytes.get_mut(4..8).unwrap().copy_from_slice(&[0, 17, 1, 0]);
    let rewritten = Savegame::decode(&bytes, LIMIT)
        .unwrap()
        .encode(Compression::None)
        .unwrap();
    assert_eq!(rewritten, bytes);
}

#[test]
fn enforces_encoded_byte_limit() {
    let original = sample();
    assert!(Savegame::decode(&original, original.len().saturating_sub(1)).is_err());
}

#[test]
fn enforces_decompressed_byte_limit() {
    let mut payload = b"DATA\0\0\x20\0".to_vec();
    payload.extend_from_slice(&[0; 8192]);
    payload.extend_from_slice(&[0; 4]);
    let parsed = Savegame::decode(&container(&payload), LIMIT).unwrap();
    for format in [Compression::Zlib, Compression::Lzma, Compression::Lzo] {
        let encoded = parsed.encode(format).unwrap();
        assert!(encoded.len() < 1024);
        assert!(Savegame::decode(&encoded, 1024).is_err());
    }
}

#[test]
fn rejects_truncated_compression_streams() {
    let parsed = Savegame::decode(&sample(), LIMIT).unwrap();
    for format in [Compression::Zlib, Compression::Lzma, Compression::Lzo] {
        let mut encoded = parsed.encode(format).unwrap();
        encoded.pop();
        assert!(Savegame::decode(&encoded, LIMIT).is_err());
    }
}

#[test]
fn rejects_trailing_compressed_data() {
    let parsed = Savegame::decode(&sample(), LIMIT).unwrap();
    for format in [Compression::Zlib, Compression::Lzma, Compression::Lzo] {
        let mut encoded = parsed.encode(format).unwrap();
        encoded.push(0xff);
        assert!(Savegame::decode(&encoded, LIMIT).is_err());
    }
}

#[test]
fn rejects_corrupt_lzo_checksum() {
    let mut encoded = Savegame::decode(&sample(), LIMIT)
        .unwrap()
        .encode(Compression::Lzo)
        .unwrap();
    *encoded.get_mut(8).unwrap() ^= 1;
    assert!(Savegame::decode(&encoded, LIMIT).is_err());
}

#[test]
fn reads_gamma_length_boundaries() {
    let cases: &[(usize, &[u8])] = &[
        (126, &[0x7f]),
        (127, &[0x80, 0x80]),
        (16382, &[0xbf, 0xff]),
        (16383, &[0xc0, 0x40, 0]),
    ];
    for &(body_len, gamma) in cases {
        let mut payload = b"ARRY\x01".to_vec();
        payload.extend_from_slice(gamma);
        payload.resize(payload.len().checked_add(body_len).unwrap(), 0x55);
        payload.extend_from_slice(&[0; 5]);
        let original = container(&payload);
        let rewritten = Savegame::decode(&original, LIMIT)
            .unwrap()
            .encode(Compression::None)
            .unwrap();
        assert_eq!(rewritten, original);
    }
}

#[test]
fn upstream_fixtures_retain_all_payloads_in_every_format() {
    let fixtures: &[(&[u8], u16)] = &[
        (include_bytes!("../../../fixtures/generated-v362.sav"), 362),
        (
            include_bytes!("../../../fixtures/upstream-regression-v308.sav"),
            308,
        ),
        (
            include_bytes!("../../../fixtures/upstream-stationlist-v211.sav"),
            211,
        ),
    ];
    for &(bytes, expected_version) in fixtures {
        let original = Savegame::decode(bytes, ottd_save::DEFAULT_MAX_BYTES).unwrap();
        assert_eq!(original.version(), expected_version);
        let uncompressed = original.encode(Compression::None).unwrap();
        for format in FORMATS {
            let encoded = original.encode(format).unwrap();
            let rewritten = Savegame::decode(&encoded, ottd_save::DEFAULT_MAX_BYTES).unwrap();
            assert_eq!(rewritten.encode(Compression::None).unwrap(), uncompressed);
        }
    }
}

proptest! {
    #[test]
    fn arbitrary_input_never_panics(bytes in prop::collection::vec(any::<u8>(), 0..4096)) {
        let result = Savegame::decode(&bytes, LIMIT);
        if let Ok(save) = result {
            prop_assert!(save.encode(Compression::None).is_ok());
        }
    }

    #[test]
    fn arbitrary_chunk_payload_never_panics(payload in prop::collection::vec(any::<u8>(), 0..4096)) {
        let original = container(&payload);
        if let Ok(save) = Savegame::decode(&original, LIMIT) {
            prop_assert_eq!(save.encode(Compression::None).unwrap(), original);
        }
    }

    #[test]
    fn arbitrary_compressed_payload_never_panics(
        payload in prop::collection::vec(any::<u8>(), 0..512),
        tag in prop::sample::select(vec![*b"OTTZ", *b"OTTX", *b"OTTD"]),
    ) {
        let mut bytes = container(&payload);
        bytes.get_mut(..4).unwrap().copy_from_slice(&tag);
        if let Ok(save) = Savegame::decode(&bytes, LIMIT) {
            prop_assert!(save.encode(Compression::None).is_ok());
        }
    }

    #[test]
    fn arbitrary_riff_body_is_preserved(body in prop::collection::vec(any::<u8>(), 0..4096)) {
        let length = u32::try_from(body.len()).unwrap().to_be_bytes();
        let mut payload = b"TEST".to_vec();
        payload.extend_from_slice(&length);
        payload.extend_from_slice(&body);
        payload.extend_from_slice(&[0; 4]);
        let original = container(&payload);
        let rewritten = Savegame::decode(&original, LIMIT).unwrap().encode(Compression::None).unwrap();
        prop_assert_eq!(rewritten, original);
    }
}
