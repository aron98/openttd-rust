// SPDX-License-Identifier: GPL-2.0-only
// OpenTTD 14ec60f248547d4d062a1160f0fc26d742319888, newgrf_stringmapping.cpp and generated strings.h.
pub(super) fn map(id: u16) -> u32 {
    match id {
        0x000E..=0x002D => u32::from(id).saturating_sub(0x000E).saturating_add(4),
        0x002E..=0x004D => u32::from(id).saturating_sub(0x002E).saturating_add(36),
        0x006E..=0x008D => u32::from(id).saturating_sub(0x006E).saturating_add(68),
        0x008E..=0x00AD => u32::from(id).saturating_sub(0x008E).saturating_add(101),
        0x00D1..=0x00E0 => u32::from(id).saturating_sub(0x00D1).saturating_add(141),
        0x200F..=0x201F => u32::from(id).saturating_sub(0x200F).saturating_add(8192),
        0x2036..=0x2041 => u32::from(id).saturating_sub(0x2036).saturating_add(8209),
        0x2059..=0x205C => u32::from(id).saturating_sub(0x2059).saturating_add(8221),
        0x4802..=0x4826 => u32::from(id).saturating_sub(0x4802).saturating_add(18432),
        0x482D..=0x482E => u32::from(id).saturating_sub(0x482D).saturating_add(731),
        0x4832..=0x4834 => u32::from(id).saturating_sub(0x4832).saturating_add(733),
        0x4835..=0x4838 => u32::from(id).saturating_sub(0x4835).saturating_add(739),
        0x4839..=0x483A => u32::from(id).saturating_sub(0x4839).saturating_add(744),
        0x004e..=0x006d => [
            138, 134, 136, 135, 137, 138, 139, 136, 136, 136, 136, 135, 136, 136, 136, 135, 136,
            136, 135, 137, 136, 137, 136, 138, 135, 137, 136, 138, 136, 138, 137, 138,
        ]
        .get(usize::from(id.saturating_sub(0x004e)))
        .copied()
        .unwrap_or(1),
        0x4830 => 3983,
        0x4831 => 3995,
        0x483B => 3994,
        0xd000..=0xffff => 2,
        _ => 1,
    }
}
