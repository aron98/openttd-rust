// SPDX-License-Identifier: GPL-2.0-only
// OpenTTD 14ec60f: newgrf_stringmapping.cpp; source-generated named StringID ranges.
use super::load_strings::{StringKey, StringTable};

pub(super) fn map_string(strings: &StringTable, grfid: u32, source: u16) -> u32 {
    match source {
        0xd000..=0xd7ff => strings.lookup(StringKey {
            grfid,
            local_id: u32::from(source & !0x400),
        }),
        0xd800..=0xffff => strings.lookup(StringKey {
            grfid,
            local_id: u32::from(source),
        }),
        0x000E..=0x002D => u32::from(source.saturating_sub(0x000E)).saturating_add(4),
        0x002E..=0x004D => u32::from(source.saturating_sub(0x002E)).saturating_add(36),
        0x006E..=0x008D => u32::from(source.saturating_sub(0x006E)).saturating_add(68),
        0x008E..=0x00AD => u32::from(source.saturating_sub(0x008E)).saturating_add(101),
        0x00D1..=0x00E0 => u32::from(source.saturating_sub(0x00D1)).saturating_add(141),
        0x200F..=0x201F => u32::from(source.saturating_sub(0x200F)).saturating_add(8192),
        0x2036..=0x2041 => u32::from(source.saturating_sub(0x2036)).saturating_add(8209),
        0x2059..=0x205C => u32::from(source.saturating_sub(0x2059)).saturating_add(8221),
        0x4802..=0x4826 => u32::from(source.saturating_sub(0x4802)).saturating_add(18432),
        0x482D..=0x482E => u32::from(source.saturating_sub(0x482D)).saturating_add(731),
        0x4832..=0x4834 => u32::from(source.saturating_sub(0x4832)).saturating_add(733),
        0x4835..=0x4838 => u32::from(source.saturating_sub(0x4835)).saturating_add(739),
        0x4839..=0x483A => u32::from(source.saturating_sub(0x4839)).saturating_add(744),
        0x004e | 0x0053 | 0x0065 | 0x0069 | 0x006b | 0x006d => 138,
        0x004f => 134,
        0x0050 | 0x0055 | 0x0056 | 0x0057 | 0x0058 | 0x005a | 0x005b | 0x005c | 0x005e | 0x005f
        | 0x0062 | 0x0064 | 0x0068 | 0x006a => 136,
        0x0051 | 0x0059 | 0x005d | 0x0060 | 0x0066 => 135,
        0x0052 | 0x0061 | 0x0063 | 0x0067 | 0x006c => 137,
        0x0054 => 139,
        0x4830 => 3983,
        0x4831 => 3995,
        0x483B => 3994,
        _ => 1,
    }
}
