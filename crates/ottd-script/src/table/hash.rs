//! Native C++ standard-library string hashes for the admitted 64-bit targets.
//! `CityHash` (Google, MIT) and `MurmurHash64A` (Austin Appleby, public domain).
//! Native headers determine ABI parameters; no libstdc++ code is incorporated.
use std::num::Wrapping as W;

mod city;

pub(super) fn string(bytes: &[u8]) -> Result<u64, crate::VmError> {
    if !cfg!(all(target_pointer_width = "64", target_endian = "little")) {
        return Err(crate::VmError::UnsupportedTableHashTarget);
    }
    if cfg!(target_vendor = "apple") {
        Ok(city::hash(bytes))
    } else if cfg!(all(target_os = "linux", target_env = "gnu")) {
        Ok(murmur(bytes))
    } else {
        Err(crate::VmError::UnsupportedTableHashTarget)
    }
}
fn murmur(bytes: &[u8]) -> u64 {
    let multiplier = W(0xc6a4_a793_5bd1_e995_u64);
    let mut hash = W(0xc70f_6907_u64) ^ (W(bytes.len() as u64) * multiplier);
    let mut chunks = bytes.chunks_exact(8);
    for chunk in &mut chunks {
        let mut word = W(word(chunk));
        word *= multiplier;
        word ^= word >> 47;
        word *= multiplier;
        hash ^= word;
        hash *= multiplier;
    }
    let tail = chunks.remainder();
    if !tail.is_empty() {
        hash ^= W(word(tail));
        hash *= multiplier;
    }
    hash ^= hash >> 47;
    hash *= multiplier;
    hash ^= hash >> 47;
    hash.0
}
fn word(bytes: &[u8]) -> u64 {
    let mut value = 0_u64;
    for (shift, byte) in (0..64).step_by(8).zip(bytes) {
        value |= u64::from(*byte) << shift;
    }
    value
}

#[cfg(test)]
mod tests {
    use std::fmt::Write;
    #[test]
    #[ignore = "requires freshly executed bundled-native table hash capture"]
    fn native_string_hashes() {
        let path = std::env::var("OTTD_SCRIPT_TABLE_HASHES").expect("native capture path");
        let native = std::fs::read_to_string(path).expect("native hashes");
        let mut actual = String::new();
        for length in 0..=4096 {
            for seed in 0..8_u64 {
                let mut state = seed.wrapping_add(1);
                let bytes: Vec<u8> = (0..length)
                    .map(|_| {
                        state ^= state << 13;
                        state ^= state >> 7;
                        state ^= state << 17;
                        state.to_le_bytes()[0]
                    })
                    .collect();
                writeln!(
                    actual,
                    "{length} {seed} {}",
                    super::string(&bytes).expect("admitted native hash ABI")
                )
                .expect("format hash");
            }
        }
        assert_eq!(actual, native);
    }
}
