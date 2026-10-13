//! Typed diagnostic events, deliberately separate from saved simulation state.
use serde::Serialize;

use super::Kind;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Fault {
    DuplicateSample,
    UnknownOwner,
    IdentityChanged,
    PublicationBeforeWait,
    NestedCandidate,
    MissingCandidate,
    ClockRead,
    WaitOrder,
    SampleShape,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub(super) struct Clock {
    pub tick: i64,
    pub economy_date: i64,
    pub economy_fract: i64,
    pub pause: i64,
}
#[derive(Debug, Clone, Copy, Serialize)]
pub(super) struct Identity {
    pub lifetime: u64,
    pub saved_job: u32,
}
#[derive(Debug, Serialize)]
pub(super) struct Event {
    pub sequence: usize,
    pub host: u64,
    pub candidate: Option<u64>,
    pub kind: Kind,
    pub clock: Option<Clock>,
    pub owner: Option<Identity>,
    pub due: Option<bool>,
    pub completed: Option<bool>,
}
