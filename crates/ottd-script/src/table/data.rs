//! Ordinary destruction detaches the delegate before dropping physical own nodes.
use super::nodes::Nodes;
use crate::{VmError, heap::Edge};

#[derive(Debug)]
pub(crate) struct Data {
    pub(crate) delegate: Option<Edge>,
    pub(crate) nodes: Nodes,
}
impl Data {
    pub(crate) fn new(capacity: usize) -> Result<Self, VmError> {
        Ok(Self {
            delegate: None,
            nodes: Nodes::new(capacity)?,
        })
    }
}
