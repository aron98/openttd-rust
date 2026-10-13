use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub(super) enum Operation {
    Define {
        grfid: u32,
        local_id: u32,
        language: u8,
        new_scheme: bool,
        newlines: bool,
        raw: Vec<u8>,
        default_id: u32,
    },
    Lookup {
        grfid: u32,
        local_id: u32,
    },
    Select {
        language: u8,
    },
    Read {
        id: u32,
    },
    Reset,
    CapacityFixture {
        count: u32,
        grfid: u32,
        raw: Vec<u8>,
        default_id: u32,
        language: u8,
    },
}

pub(super) struct Case {
    pub name: &'static str,
    pub operations: Vec<Operation>,
}

fn define(local_id: u32, language: u8, new_scheme: bool, raw: &[u8], default_id: u32) -> Operation {
    Operation::Define {
        grfid: 7,
        local_id,
        language,
        new_scheme,
        newlines: true,
        raw: raw.to_vec(),
        default_id,
    }
}

pub(super) fn cases() -> Vec<Case> {
    let mut masks = Vec::new();
    for language in 0..=255 {
        masks.push(Operation::Reset);
        masks.push(define(0xd800, language, false, b"mask", 2));
        masks.push(Operation::Lookup {
            grfid: 7,
            local_id: 0xd800,
        });
    }
    let mut languages = Vec::new();
    for language in 0..=255 {
        languages.push(define(0xd800, language, true, &[language], 2));
        languages.push(Operation::Select { language });
        languages.push(Operation::Read { id: 0x20000 });
    }
    let mut references = Vec::new();
    for id in [
        0_u16, 0xe, 0x483b, 0xcfff, 0xd000, 0xd3ff, 0xd400, 0xd7ff, 0xd800, 0xdfff, 0xe000, 0xffff,
    ] {
        references.push(define(u32::from(id), 1, true, b"before", 2));
        let [low, high] = id.to_le_bytes();
        references.push(define(u32::from(id), 1, true, &[0x81, low, high], 29));
    }
    let mut builtins = Vec::new();
    for tab in 0..32_u32 {
        if tab == 26 {
            continue;
        }
        for offset in [0, 1, 2047] {
            builtins.push(Operation::Read {
                id: (tab << 11) | offset,
            });
        }
    }
    let mut result = vec![
        Case {
            name: "old-language-masks",
            operations: masks,
        },
        Case {
            name: "all-new-language-ids",
            operations: languages,
        },
        Case {
            name: "inline-identities",
            operations: references,
        },
        Case {
            name: "builtin-table-edges",
            operations: builtins,
        },
    ];
    result.extend(selection_cases());
    result.extend(identity_cases());
    result
}

fn selection_cases() -> Vec<Case> {
    vec![
        Case {
            name: "fallback-order",
            operations: vec![
                define(0xd800, 0, true, b"American", 2),
                define(0xd800, 1, true, b"English", 29),
                Operation::Select { language: 2 },
                Operation::Read { id: 0x20000 },
                define(0xd800, 0x7f, true, b"unspecified", 39),
                Operation::Read { id: 0x20000 },
                define(0xd800, 2, true, b"", 49),
                Operation::Read { id: 0x20000 },
                Operation::Select { language: 3 },
                Operation::Read { id: 0x20000 },
                define(0xd800, 0x7f, true, b"replacement", 59),
                Operation::Read { id: 0x20000 },
            ],
        },
        Case {
            name: "custom-default-chain",
            operations: vec![
                define(0xd800, 3, true, b"French", 2),
                define(0xd801, 3, true, b"French2", 0x20000),
                Operation::Select { language: 4 },
                Operation::Read { id: 0x20001 },
                Operation::Select { language: 3 },
                Operation::Read { id: 0x20001 },
            ],
        },
        Case {
            name: "forward-reference-not-backpatched",
            operations: vec![
                define(0xd800, 1, true, &[0x81, 1, 0xd8], 2),
                define(0xd801, 1, true, b"later", 2),
                Operation::Read { id: 0x20000 },
                define(0xd801, 1, true, &[0x81, 0, 0xd8], 2),
                Operation::Read { id: 0x20001 },
            ],
        },
    ]
}

fn identity_cases() -> Vec<Case> {
    vec![
        Case {
            name: "key-and-reset-order",
            operations: vec![
                define(0xffff_ffff, 1, true, b"max local", 2),
                Operation::Define {
                    grfid: 8,
                    local_id: u32::MAX,
                    language: 1,
                    new_scheme: true,
                    newlines: false,
                    raw: b"other GRFID".to_vec(),
                    default_id: 2,
                },
                define(0xffff_ffff, 1, true, b"replace", 99),
                Operation::Lookup {
                    grfid: 8,
                    local_id: u32::MAX,
                },
                Operation::Reset,
                Operation::Lookup {
                    grfid: 8,
                    local_id: u32::MAX,
                },
                define(0, 1, true, b"new load", 2),
            ],
        },
        Case {
            name: "declared-prefix-equivalence",
            operations: vec![
                define(0, 1, true, b"s", 2),
                define(1, 1, true, b"s", 2),
                define(2, 1, true, b"s", 2),
                Operation::Reset,
                Operation::CapacityFixture {
                    count: 3,
                    grfid: 7,
                    raw: b"s".to_vec(),
                    default_id: 2,
                    language: 1,
                },
            ],
        },
        Case {
            name: "native-cap-declared-boundary",
            operations: vec![
                Operation::CapacityFixture {
                    count: 524_288,
                    grfid: 7,
                    raw: b"s".to_vec(),
                    default_id: 2,
                    language: 1,
                },
                Operation::Define {
                    grfid: 8,
                    local_id: 0,
                    language: 1,
                    new_scheme: true,
                    newlines: true,
                    raw: b"must not translate".to_vec(),
                    default_id: 2,
                },
                define(0, 1, true, b"replacement", 29),
            ],
        },
    ]
}
