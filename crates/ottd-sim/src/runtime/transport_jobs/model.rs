//! Immutable copied job inputs and owned solver annotations.
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::runtime) enum Distribution {
    Manual,
    Asymmetric,
    Symmetric,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::runtime) struct Settings {
    pub distribution: Distribution,
    pub accuracy: u8,
    pub demand_distance: u8,
    pub demand_size: u8,
    pub saturation: u8,
    pub recalc_time: u16,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::runtime) struct Edge {
    pub destination: u16,
    pub capacity: u32,
    pub usage: u32,
    pub travel_time: u32,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::runtime) struct Node {
    pub station: u16,
    pub xy: u32,
    pub supply: u32,
    pub demand: u32,
    pub edges: Vec<Edge>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::runtime) struct JobInput {
    pub graph: u16,
    pub cargo: u8,
    pub width: u32,
    pub height: u32,
    pub express: bool,
    pub settings: Settings,
    pub join_date: i32,
    pub last_compression: i32,
    pub nodes: Vec<Node>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(in crate::runtime) struct JobResult {
    pub nodes: usize,
    pub edges: usize,
    pub annotations: Vec<Annotation>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(in crate::runtime) struct Annotation {
    pub undelivered_supply: u32,
    pub demands: Vec<[u32; 2]>,
    pub edge_flows: Vec<EdgeFlow>,
    pub paths: Vec<Path>,
    pub flows: Vec<Flow>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(in crate::runtime) struct EdgeFlow {
    pub destination: u16,
    pub flow: u32,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(in crate::runtime) struct Flow {
    pub origin: u16,
    pub unrestricted: u32,
    pub shares: Vec<(u32, u16)>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(in crate::runtime) struct Path {
    pub node: u16,
    pub origin: u16,
    pub parent: Option<u16>,
    pub capacity: u32,
    pub free_capacity: i32,
    pub distance: u32,
    pub flow: u32,
    pub children: u32,
}
