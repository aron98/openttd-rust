pub(super) struct Consumer<'a> {
    pub bytes: &'a [u8],
}
impl Consumer<'_> {
    pub(super) fn byte(&mut self) -> u8 {
        let value = self.bytes.first().copied().unwrap_or(0);
        self.skip(1);
        value
    }
    pub(super) fn skip(&mut self, count: usize) {
        self.bytes = self.bytes.get(count..).unwrap_or_default();
    }
    pub(super) fn word(&mut self) -> u16 {
        let value = self
            .bytes
            .get(..2)
            .and_then(|bytes| <[u8; 2]>::try_from(bytes).ok())
            .map_or(0, u16::from_le_bytes);
        self.skip(2);
        value
    }
    pub(super) fn unicode(&mut self) -> Option<u32> {
        let first = *self.bytes.first()?;
        let (length, mask, minimum) = match first {
            0..=127 => (1, 127, 0),
            192..=223 => (2, 31, 128),
            224..=239 => (3, 15, 2048),
            240..=247 => (4, 7, 65_536),
            _ => return None,
        };
        let bytes = self.bytes.get(..length)?;
        let mut value = u32::from(first & mask);
        for &byte in bytes.iter().skip(1) {
            if byte & 0xc0 != 128 {
                return None;
            }
            value = (value << 6) | u32::from(byte & 63);
        }
        if value < minimum || value > 0x0010_ffff {
            return None;
        }
        self.skip(length);
        Some(value)
    }
}

pub(super) const fn encode(value: u32) -> ([u8; 4], usize) {
    let mut bytes = [0; 4];
    let length = match value {
        0..=0x7f => 1,
        0x80..=0x7ff => 2,
        0x800..=0xffff => 3,
        0x0001_0000..=0x0010_ffff => 4,
        _ => 0,
    };
    match length {
        1 => bytes[0] = value.to_le_bytes()[0],
        2 => {
            bytes[0] = 0xc0 | ((value >> 6) & 31).to_le_bytes()[0];
            bytes[1] = 0x80 | (value & 63).to_le_bytes()[0];
        }
        3 => {
            bytes[0] = 0xe0 | ((value >> 12) & 15).to_le_bytes()[0];
            bytes[1] = 0x80 | ((value >> 6) & 63).to_le_bytes()[0];
            bytes[2] = 0x80 | (value & 63).to_le_bytes()[0];
        }
        4 => {
            bytes[0] = 0xf0 | ((value >> 18) & 7).to_le_bytes()[0];
            bytes[1] = 0x80 | ((value >> 12) & 63).to_le_bytes()[0];
            bytes[2] = 0x80 | ((value >> 6) & 63).to_le_bytes()[0];
            bytes[3] = 0x80 | (value & 63).to_le_bytes()[0];
        }
        _ => (),
    }
    (bytes, length)
}
