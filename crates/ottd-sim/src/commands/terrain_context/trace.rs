use super::Phase;

#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub(in crate::commands) struct Recorded {
    pub(in crate::commands) ordinal: usize,
    #[serde(flatten)]
    pub(in crate::commands) event: Event,
}
#[derive(Debug, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(in crate::commands) enum Event {
    ScopeEnter {
        company: u8,
        depth: u32,
        test_mode: bool,
        map_entries: usize,
    },
    ScopeLeave {
        company: u8,
        depth: u32,
        test_mode: bool,
        map_entries: usize,
    },
    Tree {
        tile: u32,
        flags: u16,
        company: u8,
    },
    Surface {
        tile: u32,
        flags: u16,
        company: u8,
        pass: u8,
    },
    Suppressed {
        town: u16,
        flags: u16,
        company: u8,
        test_mode: bool,
    },
    Applied {
        town: u16,
        flags: u16,
        company: u8,
        test_mode: bool,
        before_rating: i32,
        after_rating: i32,
        saved_rating: i32,
        have_ratings: u16,
    },
    PhaseComplete {
        phase: Phase,
    },
}
