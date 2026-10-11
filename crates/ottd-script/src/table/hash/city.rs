//! libc++ ABI 1 `CityHash64`, including its intentional 32-bit shift at lengths 4..8.
#![expect(
    clippy::many_single_char_names,
    reason = "Names follow the published CityHash state transitions"
)]
use super::{W, word};
const K0: W<u64> = W(0xc3a5_c85c_97cb_3127);
const K1: W<u64> = W(0xb492_b66f_be98_f273);
const K2: W<u64> = W(0x9ae1_6a3b_2f90_404f);
const K3: W<u64> = W(0xc949_d7c7_509e_6557);
const fn rotate(value: W<u64>, shift: u32) -> W<u64> {
    W(value.0.rotate_right(shift))
}
fn mix(value: W<u64>) -> W<u64> {
    value ^ (value >> 47)
}
fn pair(left: W<u64>, right: W<u64>) -> W<u64> {
    let multiplier = W(0x9ddf_ea08_eb38_2d69);
    let a = mix((left ^ right) * multiplier);
    mix((right ^ a) * multiplier) * multiplier
}
fn read(bytes: &[u8], offset: usize) -> W<u64> {
    W(word(bytes.get(offset..).unwrap_or_default()))
}
fn weak(bytes: &[u8], mut a: W<u64>, mut b: W<u64>) -> (W<u64>, W<u64>) {
    a += read(bytes, 0);
    b = rotate(b + a + read(bytes, 24), 21);
    let c = a;
    a += read(bytes, 8) + read(bytes, 16);
    b += rotate(a, 44);
    (a + read(bytes, 24), b + c)
}
#[expect(
    clippy::arithmetic_side_effects,
    reason = "Length branches prove all slice offsets and subtractions; hash arithmetic uses Wrapping"
)]
#[expect(
    clippy::indexing_slicing,
    reason = "Each branch proves the required 1/4/8/16/32/64 byte window"
)]
fn short(bytes: &[u8]) -> W<u64> {
    let len = bytes.len();
    let length = W(len as u64);
    if len > 8 {
        let a = read(bytes, 0);
        let b = read(bytes, len - 8);
        return pair(
            a,
            rotate(b + length, u32::try_from(len).unwrap_or_default()),
        ) ^ b;
    }
    if len >= 4 {
        let a = u32::from_le_bytes(bytes[..4].try_into().unwrap_or_default());
        let b = W(word(&bytes[len - 4..]));
        return pair(length + W(u64::from(a.wrapping_shl(3))), b);
    }
    if len > 0 {
        let y = u64::from(bytes[0]) + (u64::from(bytes[len >> 1]) << 8);
        let z = length + W(u64::from(bytes[len - 1]) << 2);
        return mix((W(y) * K2) ^ (z * K3)) * K2;
    }
    K2
}
#[expect(
    clippy::arithmetic_side_effects,
    reason = "Caller proves length 17..32; hash arithmetic uses Wrapping"
)]
fn medium(bytes: &[u8]) -> W<u64> {
    let len = bytes.len();
    let a = read(bytes, 0) * K1;
    let b = read(bytes, 8);
    let c = read(bytes, len - 8) * K2;
    let d = read(bytes, len - 16) * K0;
    pair(
        rotate(a - b, 43) + rotate(c, 30) + d,
        a + rotate(b ^ K3, 20) - c + W(len as u64),
    )
}
#[expect(
    clippy::arithmetic_side_effects,
    reason = "Caller proves length 33..64; hash arithmetic uses Wrapping"
)]
fn large(bytes: &[u8]) -> W<u64> {
    let len = bytes.len();
    let mut z = read(bytes, 24);
    let mut a = read(bytes, 0) + (W(len as u64) + read(bytes, len - 16)) * K0;
    let mut b = rotate(a + z, 52);
    let mut c = rotate(a, 37);
    a += read(bytes, 8);
    c += rotate(a, 7);
    a += read(bytes, 16);
    let vf = a + z;
    let vs = b + rotate(a, 31) + c;
    a = read(bytes, 16) + read(bytes, len - 32);
    z += read(bytes, len - 8);
    b = rotate(a + z, 52);
    c = rotate(a, 37);
    a += read(bytes, len - 24);
    c += rotate(a, 7);
    a += read(bytes, len - 16);
    let wf = a + z;
    let ws = b + rotate(a, 31) + c;
    let r = mix((vf + ws) * K2 + (wf + vs) * K0);
    mix(r * K0 + vs) * K2
}
#[expect(
    clippy::arithmetic_side_effects,
    reason = "Long branch proves length >64; exact chunks bound all offsets"
)]
#[expect(
    clippy::indexing_slicing,
    reason = "Long branch proves each suffix and exact 64-byte chunk"
)]
pub(super) fn hash(bytes: &[u8]) -> u64 {
    let len = bytes.len();
    if len <= 16 {
        return short(bytes).0;
    }
    if len <= 32 {
        return medium(bytes).0;
    }
    if len <= 64 {
        return large(bytes).0;
    }
    let mut x = read(bytes, len - 40);
    let mut y = read(bytes, len - 16) + read(bytes, len - 56);
    let mut z = pair(read(bytes, len - 48) + W(len as u64), read(bytes, len - 24));
    let mut v = weak(&bytes[len - 64..], W(len as u64), z);
    let mut w = weak(&bytes[len - 32..], y + K1, x);
    x = x * K1 + read(bytes, 0);
    for chunk in bytes[..(len - 1) & !63].chunks_exact(64) {
        x = rotate(x + y + v.0 + read(chunk, 8), 37) * K1;
        y = rotate(y + v.1 + read(chunk, 48), 42) * K1;
        x ^= w.1;
        y += v.0 + read(chunk, 40);
        z = rotate(z + w.0, 33) * K1;
        v = weak(chunk, v.1 * K1, x + w.0);
        w = weak(&chunk[32..], z + w.1, y + read(chunk, 16));
        std::mem::swap(&mut z, &mut x);
    }
    pair(pair(v.0, w.0) + mix(y) * K1 + z, pair(v.1, w.1) + x).0
}
