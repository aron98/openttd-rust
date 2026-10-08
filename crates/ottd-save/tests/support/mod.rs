#![cfg(test)]
#![expect(
    clippy::redundant_pub_crate,
    reason = "Integration-test helpers are private to this test crate"
)]
use ottd_save::{ChunkKind, Savegame};
pub(super) fn native() -> Savegame {
    Savegame::decode(
        include_bytes!("../../../../fixtures/generated-v362.sav"),
        4 * 1024 * 1024,
    )
    .unwrap()
}

pub(super) fn rewrite(id: [u8; 4], replacement: Option<&[u8]>) -> Savegame {
    let mut output = b"OTTN\x01\x6a\0\0".to_vec();
    for chunk in native().chunks() {
        let body = if chunk.id() == id {
            let Some(body) = &replacement else {
                continue;
            };
            body
        } else {
            chunk.body()
        };
        output.extend_from_slice(&chunk.id());
        match chunk.kind() {
            ChunkKind::Riff => {
                output.extend_from_slice(&u32::try_from(body.len()).unwrap().to_be_bytes());
            }
            ChunkKind::Table => output.push(3),
            ChunkKind::Array => output.push(1),
            ChunkKind::SparseArray => output.push(2),
            ChunkKind::SparseTable => output.push(4),
        }
        output.extend_from_slice(body);
    }
    output.extend_from_slice(&[0; 4]);
    Savegame::decode(&output, 4 * 1024 * 1024).unwrap()
}

fn gamma(value: usize) -> Vec<u8> {
    let value = u32::try_from(value).unwrap();
    let [a, b, c, d] = value.to_be_bytes();
    match value {
        0..=127 => vec![d],
        128..=16_383 => vec![c | 0x80, d],
        16_384..=2_097_151 => vec![b | 0xc0, c, d],
        2_097_152..=268_435_455 => vec![a | 0xe0, b, c, d],
        _ => vec![240, a, b, c, d],
    }
}

pub(super) fn records(id: [u8; 4]) -> Vec<Vec<u8>> {
    let save = native();
    let mut bytes = save.chunks().iter().find(|c| c.id() == id).unwrap().body();
    let mut out = Vec::new();
    loop {
        let (&first, rest) = bytes.split_first().unwrap();
        bytes = rest;
        let (mut n, extra) = match first {
            0..=127 => (usize::from(first), 0),
            128..=191 => (usize::from(first & 63), 1),
            192..=223 => (usize::from(first & 31), 2),
            224..=239 => (usize::from(first & 15), 3),
            _ => (0, 4),
        };
        for _ in 0..extra {
            let (&b, rest) = bytes.split_first().unwrap();
            bytes = rest;
            n = (n << 8) | usize::from(b);
        }
        if n == 0 {
            break;
        }
        let (record, rest) = bytes.split_at(n.checked_sub(1).unwrap());
        bytes = rest;
        out.push(record.to_vec());
    }
    out
}

pub(super) fn table(records: &[Vec<u8>]) -> Vec<u8> {
    let mut body = Vec::new();
    for record in records {
        body.extend(gamma(record.len().checked_add(1).unwrap()));
        body.extend(record);
    }
    body.push(0);
    body
}
