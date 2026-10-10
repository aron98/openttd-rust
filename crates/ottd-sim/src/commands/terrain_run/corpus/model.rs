use super::{PathBuf, Value};
use crate::CommandRequest;
use serde::Deserialize;

#[derive(Deserialize)]
pub(super) struct Manifest {
    pub schema_version: u32,
    pub cases: Vec<Case>,
}
#[derive(Deserialize)]
pub(super) struct Case {
    pub id: String,
    pub directory: PathBuf,
    pub domain: Domain,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Domain {
    TerrainParity,
    LoaderClampBoundary,
    EarlyCompanyGate,
    ObserverOffControl,
    UnsupportedBridge,
    UnsupportedDeity,
    UnsupportedWater,
}
#[derive(Deserialize)]
pub(super) struct Protocol {
    pub schema_version: u32,
    pub actions: Vec<Action>,
}
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub(super) enum Action {
    Command {
        ordinal: u32,
        request: CommandRequest,
    },
    Checkpoint {
        ordinal: u32,
        label: String,
    },
}
#[derive(Deserialize)]
pub(super) struct Native {
    pub schema_version: u32,
    pub actions: Vec<NativeAction>,
}
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub(super) enum NativeAction {
    Command {
        ordinal: u32,
        receipt: Value,
        native_metadata: Metadata,
    },
    Checkpoint {
        ordinal: u32,
    },
}
#[derive(Deserialize)]
pub(super) struct Metadata {
    pub tree_rating: Option<Trace>,
}
#[derive(Deserialize)]
pub(super) struct Trace {
    pub events: Value,
}
